# M6 T-TRACE (PumpSwap venue)

Instruction-trace entries (every top-level instruction, ComputeBudget included, plus every CPI at any depth; ARCH §6.2) of the worst-case PumpSwap transactions of the release short.win on the dumped PumpSwap, pump and pump-fees programs, measured on LiteSVM 0.16.0 with every invariant hook after every instruction; the limit is `MAX_INSTRUCTION_TRACE_LENGTH = 64`. The peak stack height is the deepest CPI level reached (1 = top level; the mainnet limit is 5, ARCH §6.3); the CU is the whole transaction's (the two ComputeBudget instructions cost 150 each), within ARCH §6.1's limit for it. Written by `M6_TRACE_WRITE=1 cargo test --manifest-path tests/bench/m6_cu/Cargo.toml --test t_trace` (README.md); the exact entries of each row are also asserted (their composition is pinned in `tests/litesvm/tests/trace_pumpswap_it.rs`).

Sizes (T-SIZE, ARCH §6.4): the legacy wire size (no lookup table) and the v0 wire size through the protocol ALT (A28 with its pump part: pump, PumpSwap and pump-fees, their statics and the live recipient lists with their WSOL ATAs; `m6_cu_bench::protocol_alt_addresses`), each with its account count; LiteSVM enforces neither. Every client and keeper transaction must fit v0 + ALT (≤ 1,232 B, ≤ 64 accounts). Bold: above 1,232 B. The full T-SIZE on PumpSwap (the largest extras set, a separate fee payer, both tables, executed as v0) is `tests/litesvm/tests/tx_size_pumpswap_it.rs`.

The graduation transition (§2.5) is the crank that detects the migration and rewrites the ring; the straddle's tx3 (§5.4) is that transition crank plus the borrower's own close on the pool in one transaction; the post-wipe reward claim (§3.9) is `[create_idempotent ATA(trader, WSOL), pAMM claim_cashback, sweep_trader]` — `claim_venue_rewards` was not built (the UVA-reading pattern).

| Transaction | Entries | Expected | Peak height | CU | CU limit | Legacy B / accounts | v0 + ALT B / accounts | ARCH §6.2 |
|---|---|---|---|---|---|---|---|---|
| Token-2022, 301 B, boost spent (mature): ComputeBudget ×2 + open_short, normalising | 28 | 28 | 3 | 207690 | 250000 | **1627** / 42 | 1074 / 42 | 2 + 26 (above §6.2's ≤ 23 on the normalising path) |
| Token-2022, 301 B, boost spent (mature): ComputeBudget ×2 + close_short, normalising | 22 | 22 | 3 | 173276 | 250000 | **1529** / 40 | 1038 / 40 | 2 + 20 (above §6.2's ≤ 19 on the normalising path) |
| SPL Token, 261 B (legacy261): ComputeBudget ×2 + open_short, normalising | 28 | 28 | 3 | 188612 | 250000 | **1595** / 41 | 1073 / 41 | 2 + 26 (above §6.2's ≤ 23 on the normalising path) |
| SPL Token, 261 B (legacy261): ComputeBudget ×2 + close_short, normalising | 22 | 22 | 3 | 159472 | 250000 | **1497** / 39 | 1037 / 39 | 2 + 20 (above §6.2's ≤ 19 on the normalising path) |
| Token-2022, 301 B, boost spent (mature): ComputeBudget ×2 + the costliest open | 28 | 28 | 3 | 206440 | 250000 | **1627** / 42 | 1074 / 42 | 2 + 26 (above §6.2's ≤ 23; creator change and normalisation) |
| Token-2022, 301 B, boost spent (mature): ComputeBudget ×2 + the costliest close | 22 | 22 | 3 | 172026 | 250000 | **1529** / 40 | 1038 / 40 | 2 + 20 (above §6.2's ≤ 19; creator change and normalisation) |
| Token-2022, 301 B, boost spent (mature): ComputeBudget ×2 + crank_price + liquidate (keeper) | 21 | 21 | 3 | 221377 | 300000 | **1610** / 42 | 1088 / 42 | 2 + 2 + 17 (≤ 19 on PumpSwap; §6.2) |
| Token-2022, 301 B, boost spent (mature): ComputeBudget ×2 + crank_price + expire (keeper) | 21 | 21 | 3 | 216659 | 300000 | **1610** / 42 | 1088 / 42 | 2 + 2 + 17 (≤ 19 on PumpSwap; §6.2) |
| SPL Token, 261 B (legacy261): ComputeBudget ×2 + crank_price + liquidate (keeper) | 21 | 21 | 3 | 206990 | 300000 | **1578** / 41 | 1087 / 41 | 2 + 2 + 17 (≤ 19 on PumpSwap; §6.2) |
| SPL Token, 261 B (legacy261): ComputeBudget ×2 + crank_price + expire (keeper) | 21 | 21 | 3 | 205442 | 300000 | **1578** / 41 | 1087 / 41 | 2 + 2 + 17 (≤ 19 on PumpSwap; §6.2) |
| Token-2022, 301 B, boost spent (mature): ComputeBudget ×2 + crank_price + the costliest liquidate (keeper) | 24 | 24 | 3 | 226141 | 300000 | **1610** / 42 | 1088 / 42 | 2 + 2 + 20 (above §6.2's ≤ 19; creator change and normalisation) |
| ComputeBudget ×2 + crank_price + sol_settle S3 (keeper; the buy halt) | 6 | 6 | 2 | 85230 | 150000 | 944 / 22 | 794 / 22 | 2 + 2 + 2 (§6.2) |
| ComputeBudget ×2 + crank_price (graduation transition → PumpSwap) | 6 | 6 | 2 | 32392 | 250000 | 626 / 15 | 445 / 15 | no row (§2.5; the ring rewrite, VenueStateChanged + RouteChainChanged) |
| ComputeBudget ×2 + crank_price (→ PumpSwap) + close_short (straddle tx3, borrower) | 23 | 23 | 3 | 206927 | 400000 | **1713** / 45 | 1129 / 45 | no row (§5.4 tx3; the transition crank then the borrower's close on the pool) |
| Token-2022, 301 B, boost spent (mature): ComputeBudget ×2 + create_market (Auto, PumpSwap, init set) | 17 | 17 | 3 | 134742 | 250000 | 897 / 21 | 685 / 21 | no row |
| SPL Token, 261 B (legacy261): ComputeBudget ×2 + create_market (Auto, PumpSwap, init set) | 17 | 17 | 3 | 113790 | 250000 | 897 / 21 | 685 / 21 | no row |
| Token-2022, 301 B, boost spent (mature): ComputeBudget ×2 + sweep_trader (token leg) | 6 | 6 | 2 | 24784 | 60000 | 725 / 18 | 606 / 18 | no row (§3.9; the post-wipe token leg) |
| ComputeBudget ×2 + create ATA(trader, WSOL) + pAMM claim_cashback + sweep_trader (post-wipe reward claim) | 14 | 14 | 2 | 59801 | 150000 | 948 / 24 | 767 / 24 | no row (§3.9; claim_venue_rewards was not built — the UVA-reading pattern) |
| ComputeBudget ×2 + flag_venue_upgraded ×2 (PumpSwap, pump-fees; the dumped ProgramData) | 6 | 6 | 2 | 12356 | 400000 | 378 / 7 | 352 / 7 | 2 + 2 + 2 (§6.2) |
