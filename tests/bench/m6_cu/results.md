# M6 CU bench (PumpSwap venue)

Compute units of every PumpSwap-path instruction and the `→ PumpSwap` graduation transition (ARCH §9 M6: T-CU) of the **release** short.win (`anchor build --tools-version v1.57 --arch v3`) against the **dumped mainnet** pump, pump-fees and PumpSwap programs and canonical pools (`tests/fixtures/`, groups `pamm_state`, `pamm_synthetic`, `pump_state`), measured with Mollusk 0.15.1 on account states built by the LiteSVM harness on the PumpSwap fork, with the same short.win, pump, pump-fees, SPL Token (p-token), Token-2022 and ATA ELFs; LiteSVM 0.16.0 (mainnet feature set) runs the same instruction behind a 1.4 M `SetComputeUnitLimit` as a cross-check (its own `consumed` line) and gives its instruction-trace entries, its peak stack height and the venue's own CU (its outer frames: its pump-fees, token, System and event CPIs included). Budgets are ARCH §6.1's PROPOSED transaction limits; `results.md` names the source of each. Written by `M6_CU_WRITE=1 cargo test --manifest-path tests/bench/m6_cu/Cargo.toml --test m6_cu` (README.md); the test fails when a Mollusk figure exceeds its budget, the two harnesses disagree, or a case does not execute.

| Instruction | Case | Mollusk CU | LiteSVM CU | venue CU | Budget | Budget source | Trace | ARCH §6.2 (PumpSwap) | Peak height |
|---|---|---|---|---|---|---|---|---|---|
| `create_market` | Auto, Token-2022, 301 B, boost spent (mature): PumpSwap, the init set | 134412 | 134412 | — | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, SPL Token, 261 B (legacy261): PumpSwap, the init set | 113488 | 113488 | — | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 301 B, boost active (fresh): PumpSwap, the init set | 117912 | 117912 | — | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 301 B, cashback (cashback): PumpSwap, the init set | 132910 | 132910 | — | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `open_short` | Token-2022, 301 B, boost spent (mature): first open of a wallet | 197132 | 197132 | 75648 | 250000 | §6.1 `open_short` PumpSwap | 23 | 20 (≤ 23) | 3 |
| `open_short` | SPL Token, 261 B (legacy261): first open of a wallet | 180123 | 180123 | 67533 | 250000 | §6.1 `open_short` PumpSwap | 23 | 20 (≤ 23) | 3 |
| `open_short` | SPL Token, 211 B, no creator (legacy211): first open of a wallet | 179571 | 179571 | 64026 | 250000 | §6.1 `open_short` PumpSwap | 22 | 20 (≤ 23) | 3 |
| `open_short` | Token-2022, 301 B, cashback (cashback): first open of a wallet | 199458 | 199458 | 82850 | 250000 | §6.1 `open_short` PumpSwap | 23 | 20 (≤ 23) | 3 |
| `open_short` | Token-2022, 301 B, boost spent (mature): second position of a wallet | 194371 | 194371 | 75651 | 250000 | §6.1 `open_short` PumpSwap | 22 | 20 (≤ 23) | 3 |
| `open_short` | Token-2022, 301 B, boost spent (mature): first open, normalising | 205809 | 205809 | 75648 | 250000 | §6.1 `open_short` PumpSwap | 26 | 20 (≤ 23) | 3 |
| `open_short` | Token-2022, 301 B, boost spent (mature): costliest — normalising, creator change into an empty vault, last recipients | 204559 | 204559 | 71284 | 250000 | §6.1 `open_short` PumpSwap | 26 | 20 (≤ 23) | 3 |
| `close_short` | Token-2022, 301 B, boost spent (mature) | 164094 | 164094 | 83514 | 250000 | §6.1 `close_short` curve / PumpSwap | 17 | 16 (≤ 19) | 3 |
| `close_short` | SPL Token, 261 B (legacy261) | 152836 | 152836 | 74808 | 250000 | §6.1 `close_short` curve / PumpSwap | 17 | 16 (≤ 19) | 3 |
| `close_short` | SPL Token, 211 B, no creator (legacy211) | 150793 | 150793 | 73032 | 250000 | §6.1 `close_short` curve / PumpSwap | 16 | 16 (≤ 19) | 3 |
| `close_short` | Token-2022, 301 B, cashback (cashback) | 169529 | 169529 | 87975 | 250000 | §6.1 `close_short` curve / PumpSwap | 17 | 16 (≤ 19) | 3 |
| `close_short` | Token-2022, 301 B, boost spent (mature): normalising | 172770 | 172770 | 83514 | 250000 | §6.1 `close_short` curve / PumpSwap | 20 | 16 (≤ 19) | 3 |
| `close_short` | Token-2022, 301 B, boost spent (mature): costliest — normalising, creator change into an empty vault, last recipients | 171520 | 171520 | 79150 | 250000 | §6.1 `close_short` curve / PumpSwap | 20 | 16 (≤ 19) | 3 |
| `liquidate` | Token-2022, 301 B, boost spent (mature): after a real pump (× 2.8), both legs | 185985 | 185985 | 83379 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 17 | 16 (≤ 19) | 3 |
| `expire` | Token-2022, 301 B, boost spent (mature): past the deadline, healthy | 181012 | 181012 | 83514 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 17 | 16 (≤ 19) | 3 |
| `liquidate` | SPL Token, 261 B (legacy261): after a real pump (× 2.8), both legs | 172223 | 172223 | 74807 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 17 | 16 (≤ 19) | 3 |
| `expire` | SPL Token, 261 B (legacy261): past the deadline, healthy | 169612 | 169612 | 74808 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 17 | 16 (≤ 19) | 3 |
| `liquidate` | Token-2022, 301 B, boost spent (mature): costliest — normalising, creator change, last recipients | 193809 | 193809 | 79015 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 20 | 16 (≤ 19) | 3 |
| `liquidate` | graduation path: on the migrated pool after the warm-up | 195023 | 195023 | 94085 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 17 | 16 (≤ 19) | 3 |
| `sol_settle` | Token-2022, 301 B, boost spent (mature): healthy, the no-change call | 50268 | 50268 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | Token-2022, 301 B, boost spent (mature): S3 under the buy halt (disable_flags bit 3), after s3_wait_s | 56790 | 56790 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `crank_price` | Token-2022, 301 B, boost spent (mature): roll + record (first crank of a second) | 34782 | 34782 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | SPL Token, 261 B (legacy261): roll + record (first crank of a second) | 34189 | 34189 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 301 B, boost spent (mature): record only (same second again) | 30590 | 30590 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 301 B, boost spent (mature): roll + record after a coin-creator change: the creator re-cached (M6-SEC-4) | 37894 | 37894 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | graduation transition: `PumpCurve → PumpSwap` (the pool cache, the ring rewrite, the warm-up) | 32073 | 32073 | — | 60000 | §6.1 `crank_price` | 4 | 2 (≤ 2) | 2 |
| `crank_price` | graduation transition of a market re-detected onto the curve (`None → PumpCurve`): the PumpSwap cache derived at the transition | 56166 | 56166 | — | 60000 | §6.1 `crank_price` | 4 | 2 (≤ 2) | 2 |
| `sweep_trader` | Token-2022, 301 B, boost spent (mature): token leg — a donation to the pool vault | 24469 | 24469 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 4 | — | 2 |
| `sweep_trader` | SPL Token, 261 B (legacy261): token leg — a donation to the pool vault | 21945 | 21945 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 4 | — | 2 |
| `sweep_trader` | Token-2022, 301 B, boost spent (mature): nothing to sweep | 18525 | 18525 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 2 | — | 2 |
| `flag_venue_upgraded` | PumpSwap, the dumped 10,485,760-byte ProgramData | 6026 | 6026 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | PumpSwap, the dumped ProgramData | 8438 | 8438 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
