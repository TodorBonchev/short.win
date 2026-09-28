# m3_cu — M3 CU bench, T-TRACE and stack depth on Raydium

This is the M3 compute, trace and depth gate for the Raydium venues (ARCHITECTURE §6.1–§6.4,
§8.2 T-RAY-CU / T-RAY-TRACE / T-CU / T-TRACE, §9 M3). Everything runs the **release** short.win
(`anchor build --tools-version v1.57 --arch v3`) against the **dumped mainnet** Raydium AMM v4
and CPMM programs, pools, mints and configs of `tests/fixtures/`, on the LiteSVM harness's
mainnet fork (`shortwin_litesvm_tests::fork` / `raydium`).

- **CU bench** (`tests/m3_cu.rs` → `results.md`): every Raydium-path instruction —
  `create_market`, `open_short`, `close_short`, `liquidate`, `expire`, `sol_settle`,
  `crank_price`, `flag_venue_upgraded`, `revalidate_venue` — on AMM v4 (SPL Token; AMM v4 is SPL
  Token only), CPMM with an SPL base and CPMM with a Token-2022 base, plus the CPMM variants that
  change the path (creator fee on the SOL side, WSOL as token 1). Mollusk 0.15.1 gives the figure
  the budget applies to; LiteSVM 0.16.0 runs the same instruction as a cross-check that must
  agree to the unit, and gives the instruction's trace entries, its peak stack height and the
  Raydium program's own CU (its token CPIs included). Budgets are ARCH §6.1's transaction limits;
  `results.md` names the source of each.
- **T-TRACE** (`tests/t_trace.rs` → `trace.md`): the worst-case Raydium transactions — the
  largest client open and close with their two ComputeBudget instructions and normalisation, an
  atomic sandwich of an open with the venue's own swaps, four opens of one wallet, crank batches
  whose every crank starts a route chain, the keeper's `[crank, liquidate | expire | sol_settle
  S3]`, both latch flags — each within `MAX_INSTRUCTION_TRACE_LENGTH = 64` and at its exact
  expected count, with their CU and wire sizes (legacy; v0 with the Raydium part of the protocol
  ALT, A28). Two more tests: five opens of one wallet are refused by the runtime itself
  (`MaxInstructionTraceLengthExceeded`), and four cranks fit a v0 + ALT transaction while five
  do not (U25).
- **Stack depth** (`tests/t_depth.rs` → `depth.md`): every Raydium-path instruction called
  directly and through one, two and three levels of the TEST-ONLY CPI wrapper `wrapper/` (it
  forwards one instruction, as a Squads vault transaction or an integrator's program would call
  short.win), on LiteSVM's **mainnet** feature set (`MAX_INSTRUCTION_STACK_DEPTH = 5`; Mollusk is
  not used here: it enables SIMD-0268's limit of 9).

The per-instruction call shapes (which program at which height, in order) are pinned by
`tests/litesvm/tests/trace_raydium_it.rs`, and T-LATCH on the dumped ProgramData (real loader
upgrades) by `tests/litesvm/tests/latch_raydium_it.rs`; both run in the program-tests CI job.

```sh
# from the repository root
anchor build --tools-version v1.57 --arch v3                     # target/deploy/shortwin.so (release)
cargo-build-sbf --tools-version v1.57 --arch v3 \
  --manifest-path tests/bench/m3_cu/wrapper/Cargo.toml \
  --sbf-out-dir tests/bench/m3_cu/target/deploy                  # the wrapper (t_depth only)
cargo test --manifest-path tests/bench/m3_cu/Cargo.toml --locked -- --nocapture
# SHORTWIN_SO / M3_WRAPPER_SO override the ELFs;
# M3_CU_WRITE=1, M3_TRACE_WRITE=1, M3_DEPTH_WRITE=1 rewrite results.md, trace.md, depth.md.
```

A case or transaction that does not execute, a Mollusk figure above its budget, a disagreement
of the two harnesses, a trace count other than the expected one or above 64, a peak stack
height above 3, a client or keeper transaction above 1,232 B as v0 + ALT (the v1 keeper batch:
above 4,096 B), or a wrapper level that does not reach exactly `direct + levels` fails the
bench.

## Pre-states

- Every case builds a fresh fork (`Env::ray_fork`), lists the pool with the real
  `create_market(RaydiumV4 | RaydiumCpmm)`, lends `40·Q` and waits out the listing window, where
  `Q = min(X/400, 2 SOL of notional)` (the `ray_it` size). Every transaction that builds the state
  goes through `Env::send`, so the invariant hooks run.
- The keeper exits are prepared the way a keeper decides: `liquidate` after a real whale buy
  lifts the price ≈ 2.2× (√2.2 − 1 of the SOL reserve), cranked second by second until a
  `simulate_transaction` of the keeper's `[crank_price, liquidate]` succeeds (the simulation
  commits nothing); `expire` three cranked seconds past the deadline; S3 on a pool halted by its
  admin's own instruction (AMM v4 `SetParams` status 2, CPMM `update_pool_status` 4), cranked
  every 30 s from just before the deadline until the simulated `[crank_price, sol_settle]`
  settles S3. A case therefore measures the executing path, not a refusal.
- The borrower and the sandwich attacker have **fixed** keys, so the canonical-bump searches of
  the position and exposure PDAs and the venue's key comparisons cost the same on every run: the
  reports are reproducible (the M2 bench used random keys, and an open varied by ≈ 1.5 k CU per
  extra bump iteration).
- The latch cases change the dumped ProgramData's deploy slot in place
  (`fork::set_deploy_slot`); the U5 case writes a synthetic 10,485,760-byte ProgramData (pump's
  size; pump is not dumped) at pump's compile-time address.

## Workspace and pins

A standalone workspace with its own `Cargo.lock`, like `tests/bench/m2_cu`, so Mollusk does not
enter the on-chain workspace's lockfile. The lockfile was seeded from `tests/bench/m2_cu`'s and
differs from it only in this workspace's own two packages: anchor 1.2.0, LiteSVM 0.16.0 and the
Agave runtime crates resolve to the same versions as the program tests, held at 4.2.2 (C1-14).
`wrapper/` is a workspace member whose dependencies (`solana-program-entrypoint`, `solana-cpi`
and their component crates) were already in that lockfile through anchor-lang; it is built for
SBF with `cargo-build-sbf --tools-version v1.57 --arch v3` (never deployed) and is a plain
library on the host.

## Findings (see `results.md`, `trace.md` and `depth.md` for the figures)

- **Mollusk and LiteSVM agree to the unit on all 46 cases.**
- **CU, all within ARCH §6.1:**
  - `open_short`: 98.7–130.5 k (limit 150 k). The venue CPI is 15.4 k (AMM v4) and 25.1 k /
    28.0 k (CPMM SPL / Token-2022), as R-RAY §4.3 measured, but short.win's own part is
    ≈ 83–102 k against §6.1's estimate of 44–59 k: the worst case (a normalising first open on
    Token-2022 CPMM) leaves 19.5 k of headroom.
  - `close_short` 68.3–89.2 k, `liquidate` 90.5–111.1 k, `expire` 86.1–104.5 k (limit 200 k;
    §6.1 estimated 70–107 k for the keeper exits).
  - `sol_settle` 38.4–42.0 k as a no-op, 47.4–53.1 k for S3 (limit 150 k).
  - `crank_price` 22.9–27.8 k (limit 60 k; §6.1 estimated 18–23 k).
  - `create_market` 59.2–71.2 k (limit 250 k).
  - **U5**: `flag_venue_upgraded` costs 6,032 CU and `revalidate_venue` 8,440 CU on the
    1,406,429-byte AMM v4 and 793,869-byte CPMM ProgramData **and** on a 10 MiB one: copying the
    account into the input region is not metered in CU. §6.1's 400 k request is far more than
    needed.
  - The buy leg's venue CPI (`swap_base_out_v2` on AMM v4) is 15.9–16.0 k, a little above
    R-RAY's 15.2–15.5 k range for the v2 swaps.
- **Trace:** a wallet's first open is 15 entries, a repeat borrower's 14, a normalising open 18;
  every exit is 9 (12 normalising); a crank is 2, or 3 when the pool just halted
  (`RouteChainChanged`); S3 is 2, the no-op `sol_settle` 1; `create_market` 7; the latch
  instructions 2. As M2 found on the mock, ARCH §6.2's Raydium row is low: 13 (≤ 15) for an open
  omits the position `init` and the exposure `init_if_needed`, the normalising open is 18 and
  the normalising exit 12 against the table's maxima 15 and 11, and a crank that starts a route
  chain is 3, so a keeper batch of `k` cranks is up to `2 + 3k` entries. The worst transaction
  is four opens of one wallet at 59 entries; a fifth open (73) is refused by the runtime.
- **Depth:** every Raydium trading path peaks at height 3 (short.win 1 → venue 2 → token 3).
  Two wrapper levels fit (peak 5) and a third fails with `CallDepth` at the venue's token CPI;
  `sol_settle`, `crank_price`, `create_market` and `flag_venue_upgraded` peak at 2 and fit three.
  So short.win can be CPI'd **twice** on the Raydium paths, not only once as ARCH §6.3 words it.
- **Sizes** (a T-SIZE preview): a client open or close and a keeper's `[crank, exit]`, with
  their ComputeBudget instructions, fit 1,232 B even as legacy transactions (at most 1,166 B: a
  normalising Token-2022 CPMM open), and at most 873 B as v0 + ALT. The sandwich (≤ 1,406 B
  legacy) and four opens (≤ 1,514 B legacy) need v0 + ALT (≤ 1,209 B). A v0 + ALT transaction
  holds four Raydium cranks (1,113 B; five are 1,327 B), the byte estimate of ARCH §6.4; the
  six-crank batch is 1,542 B as v0 + ALT, so it is a v1 transaction (44 accounts).
