# m5_cu — M5 CU bench, T-TRACE, T-SIZE preview, T-STRADDLE (curve part) and stack depth on the pump curve

This is the M5 compute, trace, size and depth gate for the pump bonding-curve venue (ARCHITECTURE
§6.1–§6.4, §8.2 T-CU / T-TRACE / T-SIZE / T-DEPTH / T-STRADDLE, §9 M5). Everything runs the
**release** short.win (`anchor build --tools-version v1.57 --arch v3`) against the **dumped
mainnet** pump, pump-fees and PumpSwap programs and the pump coins of `tests/fixtures/` (group
`pump_state`), on the LiteSVM harness's pump fork (`shortwin_litesvm_tests::fork` / `pump`).

- **CU bench** (`tests/m5_cu.rs` → `results.md`): every curve-path instruction —
  `create_market(Auto)` (a curve listing with the init set on the 115-, 125- and 151-byte
  layouts, a `PumpMigrating` listing, both list-only `None` listings), `open_short` (a wallet's
  first open on every layout, a repeat borrower, normalising, the costliest path), `close_short`
  (plain, normalising, the costliest path, a completing buyback), `liquidate` (after a real
  pump), `expire` (at the deadline, the costliest path, a completing buyback), `sol_settle` (the
  no-op, S3 on `PumpMigrating` at 70 min, S3 on `None` after the chain — M2-SEC-5),
  `crank_price` (roll and record; `PumpCurve → PumpMigrating`, `PumpMigrating → None`, a `None`
  market's crank, the `None → PumpCurve` re-detection), `sweep_trader`'s token leg,
  `flag_venue_upgraded` and `revalidate_venue` on the dumped 10 MiB pump and PumpSwap and the
  0.95 MB pump-fees ProgramData — on SPL Token and Token-2022 curves. Mollusk 0.15.1 gives the
  figure the budget applies to; LiteSVM 0.16.0 runs the same instruction behind a 1.4 M
  `SetComputeUnitLimit` as a cross-check that must agree to the unit (the instruction's own
  `consumed` log line) and gives its trace entries, its peak stack height and pump's own CU (its
  outermost frames, its pump-fees, token, System and event CPIs included). Budgets are ARCH
  §6.1's PROPOSED transaction limits; `results.md` names the source of each.
- **T-TRACE** (`tests/t_trace.rs` → `trace.md`): the worst-case curve transactions — the largest
  client open and close with their two ComputeBudget instructions, normalisation and the
  creator-vault top-up, a completing close, an atomic sandwich of an open with the curve's own
  trades, four opens of one wallet, a batch of four cranks that all move their market to
  `PumpMigrating`, the keeper's `[crank, liquidate | expire | sol_settle S3]` (S3 on
  `PumpMigrating` and on `None`), the three pump latch flags, a curve listing, `sweep_trader` —
  each within `MAX_INSTRUCTION_TRACE_LENGTH = 64` and at its exact expected count, within ARCH
  §6.1's CU limit, with its wire sizes (legacy; v0 through the protocol ALT of A28 **with its pump
  part**: pump, PumpSwap and pump-fees, their statics, the live `Global` / `GlobalConfig` fee and
  buyback recipients with their WSOL ATAs). Three more tests: five opens of one wallet are refused
  by the runtime itself (`MaxInstructionTraceLengthExceeded`), how many opens of one wallet fit a
  v0 transaction, and the crank batches the protocol ALT holds (the four real curve markets and
  synthetic ones beyond them, sized only).
- **T-STRADDLE, curve part** (`trace.md`'s last rows): the ≥ 3-transaction bundle of ARCH §5.4 —
  **tx1** `ComputeBudget ×2 + complete_curve(max_sol_cost)` of the `complete-curve` helper (A66,
  `programs/complete_curve`; its SBF build `$COMPLETE_CURVE_SO`, else
  `target/deploy/complete_curve.so`): it reads `rT` **at execution** and buys it with the keeper
  as pump's `user` (helper 1 → pump 2 → pump-fees / Token / System / event 3), measured on the
  curve as the keeper read it, ★ after a third party's curve buy landed between the keeper's read
  and the bundle, and as a no-op on a curve a third party completed; **tx2** `ComputeBudget ×2 +
  migrate_v2` top-level with the two boost accounts. tx3 (the position's PumpSwap exit) is M6's.
- **Stack depth** (`tests/t_depth.rs` → `depth.md`): every curve-path instruction called directly
  and through one, two and three levels of the TEST-ONLY CPI wrapper `wrapper/` (it forwards one
  instruction, as a Squads vault transaction or an integrator's program would call short.win), on
  LiteSVM's **mainnet** feature set (`MAX_INSTRUCTION_STACK_DEPTH = 5`; Mollusk is not used
  here: it enables SIMD-0268's limit of 9); and T-DEPTH's second half on the curve — the venue
  accounts pump would create one level deeper (the trader's pump UVA, `trader_base_ata`) are
  required, so a leg without them fails cleanly before pump runs (`VenueAccountMismatch`,
  `TraderStateInvalid`), direct and wrapped.

The per-instruction call shapes (which program at which height, in order) are pinned by
`tests/litesvm/tests/trace_pump_it.rs`, the full T-SIZE on the curve (the largest extras set, a
separate fee payer, both tables, executed as v0 under the invariant hooks) by
`tests/litesvm/tests/tx_size_pump_it.rs`, and T-LATCH on the dumped pump ProgramData (real
loader upgrades) by `tests/litesvm/tests/latch_pump_it.rs`; the three run in the program-tests
CI job.

```sh
# from the repository root
anchor build --tools-version v1.57 --arch v3                     # target/deploy/shortwin.so (release), complete_curve.so
cargo-build-sbf --tools-version v1.57 --arch v3 \
  --manifest-path tests/bench/m5_cu/wrapper/Cargo.toml \
  --sbf-out-dir tests/bench/m5_cu/target/deploy -- --locked      # the wrapper (t_depth, t_trace)
cargo test --manifest-path tests/bench/m5_cu/Cargo.toml --locked -- --nocapture
# SHORTWIN_SO / COMPLETE_CURVE_SO / M5_WRAPPER_SO override the ELFs;
# M5_CU_WRITE=1, M5_TRACE_WRITE=1, M5_DEPTH_WRITE=1 rewrite results.md, trace.md, depth.md.
```

A case or transaction that does not execute, a Mollusk figure above its budget, a disagreement
of the two harnesses, a trace count other than the expected one or above 64, a peak stack
height other than the expected one (above 3 for an instruction), a transaction above its §6.1
CU limit, a client or keeper transaction above 1,232 B as v0 + ALT (a transaction marked v1:
above 4,096 B or 64 keys), or a wrapper level that does not reach exactly `direct + levels`
fails the bench.

## Pre-states

- Every case builds a fresh pump fork (`Env::fork_pump_coin`), lists the curve with the real
  `create_market(Auto)` and its pump init set, lends `40·Q` and waits out the warm-up (60 s),
  then cranks once, where `Q = vT/200` (the `pump_it` size, within every default cap). The fresh
  125-byte curve first gets `vT/7` of buys by a normal user (its `rQ` is one lamport as dumped).
  Every transaction that builds the state goes through `Env::send`, so the invariant hooks run.
- The coins: `legacy115` (SPL Token, 115 B, no creator — its buys make one System transfer less),
  `t22_115` (Token-2022, 115 B, a creator), `fresh` (Token-2022, 125 B), `near` (Token-2022,
  151 B, `rT` ≈ 8.9·10¹² — the completing buyback, the migration and the straddle), and the
  graduated coins (`graduated`, a PumpSwap-class coin, listed on its canonical pool with the pAMM
  init set since M6 — `tests/bench/m6_cu` measures every PumpSwap listing; `fartcoin`, graduated
  elsewhere, the list-only listing).
- **The costliest paths** (`costliest_legs`): the curve's creator changed after the listing to a
  fixed key whose creator vault is **empty** — the adapter re-derives both creator-vault slots
  (T12, T13) from the live creator, and pump tops the new vault up to `Rent.minimum_balance(0)`
  from the trader (T-RENT; an open then sells `vT/100` so `S_exp` covers the top-up within
  `exec_tol_bps`) — the **last** fee and buyback recipients of the live `Global` (the longest key
  scans), and normalisation (lamports donated to the trader PDA and tokens to `trader_base_ata`).
- The keeper exits are prepared the way a keeper decides: `liquidate` after a real `buy_v2` of
  40 % of `vT` (the price × 2.8), cranked second by second until a `simulate_transaction` of the
  keeper's `[crank_price, liquidate]` succeeds (the simulation commits nothing); `expire` three
  cranked seconds past the deadline; the completing expiry cranked until its simulated
  `[crank, expire]` succeeds (the third party's buy-down lifts the price above the bound's
  reference until the EMA follows). S3 on `PumpMigrating`: a third party completes the curve
  30 min before the deadline, the keeper cranks and simulates `[crank, sol_settle]` every 30 s
  until S3 settles (70 min after `migration_seen_at`). S3 on `None`: the complete curve's
  reserves zeroed by a migration elsewhere (planted on pump's account, as `pump_it::graduation`),
  the keeper on the detection set every 30 s until S3 settles (`s3_wait_s` after the chain
  started).
- The borrowers, the keeper, the sandwich attacker, the creator and the changed creator have
  **fixed** keys, so the canonical-bump searches of the position and exposure PDAs and the
  adapter's re-derivations cost the same on every run: the reports are byte-identical across
  runs.
- The latch cases change the dumped ProgramData's deploy slot in place
  (`fork::set_pump_deploy_slot`); `latch_pump_it` runs the loader's real `Upgrade`.

## Workspace and pins

A standalone workspace with its own `Cargo.lock`, like `tests/bench/m3_cu`, so Mollusk does not
enter the on-chain workspace's lockfile. The lockfile was seeded from `tests/bench/m3_cu`'s and
differs from it only in this workspace's own two packages (and the bench no longer names
`litesvm-token` directly): anchor 1.2.0, LiteSVM 0.16.0 and the Agave runtime crates resolve to
the same versions as the program tests, held at 4.2.2 (C1-14). `wrapper/` is a copy of
`tests/bench/m3_cu/wrapper` under its own id (`short.win-m5-cpi-wrapper-program`), built for SBF
with `cargo-build-sbf --tools-version v1.57 --arch v3` (never deployed) and a plain library on the
host.

## Findings (see `results.md`, `trace.md` and `depth.md` for the figures)

- **Mollusk and LiteSVM agree to the unit on all 56 cases.**
- **CU, all within ARCH §6.1's PROPOSED limits, all above its estimates:**
  - `open_short` 176.5–206.0 k (limit 250 k; §6.1 estimated 126–144 k). Pump's `sell_v2` is
    69.6–85.4 k (R-PUMP §6.6 measured 81.6–84.5 k); short.win's own part is ≈ 102–121 k against the
    estimate's 44–59 k. The costliest open (normalising, a creator change into an empty vault,
    the last recipients, the 151-byte curve) leaves 44 k of headroom.
  - `close_short` 156.7–187.0 k (limit 250 k; estimated 123–155 k); pump's `buy_v2` 80.4–97.9 k
    (the completing buy the most). `liquidate` 177.5–207.8 k, `expire` 173.3–204.3 k (limit
    300 k; estimated 141–167 k).
  - `sol_settle` 42.1–42.5 k as a no-op, 64.7 k for S3 on `PumpMigrating` (FV on the
    post-migration pool) and 45.4 k on `None` (limit 150 k).
  - `crank_price` 26.6–31.5 k for a roll and record, 36.6 k for `PumpCurve → PumpMigrating`,
    26.1 k for `PumpMigrating → None`, 16.6 k on a `None` market, and **50.7 k for the
    `None → PumpCurve` re-detection** (it writes the curve cache and starts the price state):
    within the 60 k limit with 9.3 k of headroom, well above §6.1's 18–23 k estimate.
  - `create_market(Auto)` 104.8–131.7 k for a curve listing (the ATA program's creation of
    `trader_base_ata` and pump's `init_user_volume_accumulator` included; above §6.1's 70–120 k
    for Token-2022 curves), 53.3–62.2 k for a list-only listing (limit 250 k).
  - `sweep_trader`'s token leg 22.8–29.8 k (no §6.1 row; the 60 k lender class).
  - **U5 on the real accounts:** `flag_venue_upgraded` 6,026 CU and `revalidate_venue` 8,438 CU on
    the dumped 10,485,760-byte pump and PumpSwap ProgramData and the 948,989-byte pump-fees one,
    as on Raydium's: the input-region copy is not metered.
- **Trace** (ARCH §6.2's curve column: open 12, exits 12, each ≤ 15): an open is **14** entries
  for a wallet's first position (the position and exposure inits, as on Raydium), 13 for a repeat
  borrower, **17 normalising**, **18 with pump's creator-vault top-up**; an exit is 12 on a curve
  without a creator fee and 13 with one (pump's creator transfer), **15 / 16 normalising**,
  **17 with the top-up**, and a completing buyback adds pump's completion event and short.win's
  `VenueStateChanged` (15). So the table's maxima of 15 are exceeded by the normalising and
  top-up paths. A crank is 2, or **4** when it moves the market to `PumpMigrating` or
  re-detects a curve (`VenueStateChanged`, `RouteChainChanged`) and 3 for `PumpMigrating → None`:
  a keeper batch of `k` curve cranks is up to `2 + 4k` entries. A curve listing is 15 (peak 3);
  `sol_settle` S3 is 2, the no-op 1; `sweep_trader`'s token leg 4. Four opens of one wallet are
  55 entries; a fifth (68) is refused by the runtime. Since pump's 2026-10-08 build the first
  sale on the 151-byte curve also grows it to 166 bytes (one System transfer more,
  `NEAR_GROWTH`): the costliest open is 2 + 19, four opens 56 and five 69.
- **Sizes** (v0 through A28 with its pump part; the largest-extras worst case is
  `tx_size_pump_it`'s): a client open is 1,044–1,045 B / 42–43 accounts with its two
  ComputeBudget instructions (§6.4 estimated ≈ 900–980 B); a close 1,008–1,009 B; the keeper's
  `[crank, liquidate | expire]` 1,058–1,059 B; S3 on `PumpMigrating` 732 B, on `None` 686 B; a
  curve listing 685 B. **Only two opens of one wallet fit a v0 transaction** (1,175 B; three are
  1,305 B, four 1,435 B — a v1 transaction holds four at 2,050 B / 46 keys; Raydium fits four in
  v0). The atomic sandwich of an open with the curve's own trades is 1,410–1,411 B: an attacker
  needs v1 (2,087–2,119 B / 51–52 keys) or its own table, which it has. A v0 transaction holds
  **five curve cranks** (1,181 B; six are 1,365 B), so the keeper's six-crank batch is a v1
  transaction (1,546 B / 40 keys), as §6.4's table says.
- **Depth:** every curve leg peaks at height 3 (short.win 1 → pump 2 → pump-fees / Token /
  System / pump's event 3), and so does a curve listing (short.win 1 → the ATA program / pump 2 →
  Token / System / pump's event 3): two wrapper levels fit (peak 5), a third fails with
  `CallDepth`. `sol_settle`, `crank_price`, `sweep_trader` and `flag_venue_upgraded` peak at 2 and
  fit three. The `complete_curve` helper also peaks at 3, as §5.4 says: two wrapper levels fit
  (peak 5), a third fails with `CallDepth`.
- **T-STRADDLE (curve part):** tx1 (the helper) is 12 entries (§6.2: ≈ 11), 110.4 k CU, 763 B
  as v0 + ALT (1,192 B legacy), peak 3 — the same (12 entries, 110.4 k CU) when a third party's
  curve buy landed between the keeper's read and the bundle (★: the helper completes at the live
  `rT`), and 3 entries, 10.7 k CU, peak 1 as a no-op on a completed curve (no CPI; the helper's
  own cost, Anchor's dispatch and the curve PDA check, against §6.1's "< 2 k + 3–5 k"); tx2
  `migrate_v2` is exactly **53** entries (§5.4: 51 + 2), peak **4**
  (A15), 329.3 k CU without its ComputeBudget instructions (within ECON T7/T8's
  322,371–358,072), 912 B as v0 + ALT.
