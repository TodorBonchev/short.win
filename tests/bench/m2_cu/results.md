# M2 CU bench

Compute units of every M2 instruction (ARCH §9 M2) on short.win's **test-venue** SBF build (`scripts/build-test-venue.sh`: `anchor build --tools-version v1.57 --arch v3 -- --features test-venue`) against the TEST-ONLY mock venue, measured with Mollusk 0.15.1 on account states built by the LiteSVM harness, with the same SPL Token (p-token), Token-2022 and mock ELFs; LiteSVM 0.16.0 runs the same instruction as a cross-check and gives its instruction-trace entries. Written by `M2_CU_WRITE=1 cargo test --manifest-path tests/bench/m2_cu/Cargo.toml` (README.md); the test fails when a Mollusk figure exceeds its budget, the two harnesses disagree, or a case does not execute.

| Instruction | Case | Mollusk CU | LiteSVM CU | Budget | Budget source | Trace | ARCH §6.2 (Raydium) |
|---|---|---|---|---|---|---|---|
| `open_short` | first open of a wallet, SPL Token base (position + exposure created) | 100475 | 100475 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) |
| `open_short` | first open of a wallet, Token-2022 base | 105394 | 105394 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) |
| `open_short` | second position of a wallet (exposure exists), SPL | 96656 | 96656 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 14 | 13 (≤ 15) |
| `open_short` | with normalisation (trader and trader_wsol donated to), SPL | 110959 | 110959 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 18 | 13 (≤ 15) |
| `close_short` | SPL Token base | 69554 | 69554 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) |
| `close_short` | Token-2022 base | 71676 | 71676 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) |
| `close_short` | with normalisation, SPL | 76049 | 76049 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 12 | 9 (≤ 11) |
| `add_margin` | 1 SOL to an open position | 13449 | 13449 | 60000 | no §6.1 row; §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 3 | — |
| `liquidate` | price ×2.3 held 40 s: both legs, fresh reference, within the fixed bound | 94940 | 94940 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) |
| `expire` | past the deadline, not CR-liquidatable, within the expiry bound | 90338 | 90338 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) |
| `sol_settle` | healthy position: the no-change call (a keeper's per-second crank) | 37150 | 37150 | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) |
| `sol_settle` | S3: expired, route flagged, chain unbroken for s3_wait_s | 56908 | 56908 | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) |
| `crank_price` | roll + record (first crank of a second), SPL | 24621 | 24621 | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) |
| `crank_price` | record only (same second again) | 19728 | 19728 | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) |
| `crank_price` | roll + record, Token-2022 base | 24625 | 24625 | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) |
| `crank_price` | venue halted: roll, no record, both chains start (+ RouteChainChanged) | 25081 | 25081 | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) |
| `crank_price` | list-only (`None`) market, detection set | 17760 | 17760 | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) |
| `set_route_broken` | one market | 7799 | 7799 | 60000 | no §6.1 row; admin flag class (M1 bench) | 2 | — |
| `set_route_broken` | venue bit (global) | 5828 | 5828 | 60000 | no §6.1 row; admin flag class (M1 bench) | 2 | — |
| `set_venue_extras` | 4 templates × 4 seeds (the largest set) | 13192 | 13192 | 200000 | no §6.1 row; admin class (M1 bench) | 2 | — |
| `set_venue_extras` | rotate current → previous, 4 × 4 | 13172 | 13172 | 200000 | no §6.1 row; admin class (M1 bench) | 2 | — |
| `set_venue_extras` | rotate a week-old current → stable and previous, 4 × 4 (M2-SEC-2) | 13190 | 13190 | 200000 | no §6.1 row; admin class (M1 bench) | 2 | — |
| `sweep_trader` | trader, trader_wsol and ATA(trader, WSOL) donated to; S > 0 (vests) (M2-SEC-4) | 26318 | 26318 | 60000 | no §6.1 row (§3.9, M2-SEC-4); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 5 | — |
