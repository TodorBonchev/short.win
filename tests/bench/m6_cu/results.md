# M6 CU bench (PumpSwap venue)

Compute units of every PumpSwap-path instruction and the `→ PumpSwap` graduation transition (ARCH §9 M6: T-CU) of the **release** short.win (`anchor build --tools-version v1.57 --arch v3`) against the **dumped mainnet** pump, pump-fees and PumpSwap programs and canonical pools (`tests/fixtures/`, groups `pamm_state`, `pamm_synthetic`, `pump_state`), measured with Mollusk 0.15.1 on account states built by the LiteSVM harness on the PumpSwap fork, with the same short.win, pump, pump-fees, SPL Token (p-token), Token-2022 and ATA ELFs; LiteSVM 0.16.0 (mainnet feature set) runs the same instruction behind a 1.4 M `SetComputeUnitLimit` as a cross-check (its own `consumed` line) and gives its instruction-trace entries, its peak stack height and the venue's own CU (its outer frames: its pump-fees, token, System and event CPIs included). Budgets are ARCH §6.1's PROPOSED transaction limits; `results.md` names the source of each. Written by `M6_CU_WRITE=1 cargo test --manifest-path tests/bench/m6_cu/Cargo.toml --test m6_cu` (README.md); the test fails when a Mollusk figure exceeds its budget, the two harnesses disagree, or a case does not execute.

| Instruction | Case | Mollusk CU | LiteSVM CU | venue CU | Budget | Budget source | Trace | ARCH §6.2 (PumpSwap) | Peak height |
|---|---|---|---|---|---|---|---|---|---|
| `create_market` | Auto, Token-2022, 301 B, boost spent (mature): PumpSwap, the init set | 136625 | 136625 | — | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, SPL Token, 261 B (legacy261): PumpSwap, the init set | 113713 | 113713 | — | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 301 B, boost active (fresh): PumpSwap, the init set | 120055 | 120055 | — | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 301 B, cashback (cashback): PumpSwap, the init set | 135020 | 135020 | — | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `open_short` | Token-2022, 301 B, boost spent (mature): first open of a wallet | 201334 | 201334 | 77075 | 250000 | §6.1 `open_short` PumpSwap | 23 | 20 (≤ 23) | 3 |
| `open_short` | SPL Token, 261 B (legacy261): first open of a wallet | 183270 | 183270 | 68482 | 250000 | §6.1 `open_short` PumpSwap | 23 | 20 (≤ 23) | 3 |
| `open_short` | SPL Token, 211 B, no creator (legacy211): first open of a wallet | 182631 | 182631 | 64912 | 250000 | §6.1 `open_short` PumpSwap | 22 | 20 (≤ 23) | 3 |
| `open_short` | Token-2022, 301 B, cashback (cashback): first open of a wallet | 203724 | 203724 | 84274 | 250000 | §6.1 `open_short` PumpSwap | 23 | 20 (≤ 23) | 3 |
| `open_short` | Token-2022, 301 B, boost spent (mature): second position of a wallet | 198845 | 198845 | 77078 | 250000 | §6.1 `open_short` PumpSwap | 22 | 20 (≤ 23) | 3 |
| `open_short` | Token-2022, 301 B, boost spent (mature): first open, normalising | 210451 | 210451 | 77075 | 250000 | §6.1 `open_short` PumpSwap | 26 | 20 (≤ 23) | 3 |
| `open_short` | Token-2022, 301 B, boost spent (mature): costliest — normalising, creator change into an empty vault, last recipients | 209201 | 209201 | 72711 | 250000 | §6.1 `open_short` PumpSwap | 26 | 20 (≤ 23) | 3 |
| `close_short` | Token-2022, 301 B, boost spent (mature) | 165915 | 165915 | 84908 | 250000 | §6.1 `close_short` curve / PumpSwap | 17 | 16 (≤ 19) | 3 |
| `close_short` | SPL Token, 261 B (legacy261) | 153613 | 153613 | 75772 | 250000 | §6.1 `close_short` curve / PumpSwap | 17 | 16 (≤ 19) | 3 |
| `close_short` | SPL Token, 211 B, no creator (legacy211) | 151499 | 151499 | 73933 | 250000 | §6.1 `close_short` curve / PumpSwap | 16 | 16 (≤ 19) | 3 |
| `close_short` | Token-2022, 301 B, cashback (cashback) | 171169 | 171169 | 89368 | 250000 | §6.1 `close_short` curve / PumpSwap | 17 | 16 (≤ 19) | 3 |
| `close_short` | Token-2022, 301 B, boost spent (mature): normalising | 175469 | 175469 | 84908 | 250000 | §6.1 `close_short` curve / PumpSwap | 20 | 16 (≤ 19) | 3 |
| `close_short` | Token-2022, 301 B, boost spent (mature): costliest — normalising, creator change into an empty vault, last recipients | 174219 | 174219 | 80544 | 250000 | §6.1 `close_short` curve / PumpSwap | 20 | 16 (≤ 19) | 3 |
| `liquidate` | Token-2022, 301 B, boost spent (mature): after a real pump (× 2.8), both legs | 189199 | 189199 | 84773 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 17 | 16 (≤ 19) | 3 |
| `expire` | Token-2022, 301 B, boost spent (mature): past the deadline, healthy | 185006 | 185006 | 84908 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 17 | 16 (≤ 19) | 3 |
| `liquidate` | SPL Token, 261 B (legacy261): after a real pump (× 2.8), both legs | 174793 | 174793 | 75771 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 17 | 16 (≤ 19) | 3 |
| `expire` | SPL Token, 261 B (legacy261): past the deadline, healthy | 172810 | 172810 | 75772 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 17 | 16 (≤ 19) | 3 |
| `liquidate` | Token-2022, 301 B, boost spent (mature): costliest — normalising, creator change, last recipients | 197380 | 197380 | 80409 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 20 | 16 (≤ 19) | 3 |
| `liquidate` | graduation path: on the migrated pool after the warm-up | 198543 | 198543 | 95479 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 17 | 16 (≤ 19) | 3 |
| `sol_settle` | Token-2022, 301 B, boost spent (mature): healthy, the no-change call | 52360 | 52360 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | Token-2022, 301 B, boost spent (mature): S3 under the buy halt (disable_flags bit 3), after s3_wait_s | 58192 | 58192 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `crank_price` | Token-2022, 301 B, boost spent (mature): roll + record (first crank of a second) | 35030 | 35030 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | SPL Token, 261 B (legacy261): roll + record (first crank of a second) | 34280 | 34280 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 301 B, boost spent (mature): record only (same second again) | 30838 | 30838 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 301 B, boost spent (mature): roll + record after a coin-creator change: the creator re-cached (M6-SEC-4) | 38142 | 38142 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | graduation transition: `PumpCurve → PumpSwap` (the pool cache, the ring rewrite, the warm-up) | 32847 | 32847 | — | 60000 | §6.1 `crank_price` | 4 | 2 (≤ 2) | 2 |
| `crank_price` | graduation transition of a market re-detected onto the curve (`None → PumpCurve`): the PumpSwap cache derived at the transition | 56274 | 56274 | — | 60000 | §6.1 `crank_price` | 4 | 2 (≤ 2) | 2 |
| `sweep_trader` | Token-2022, 301 B, boost spent (mature): token leg — a donation to the pool vault | 24780 | 24780 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 4 | — | 2 |
| `sweep_trader` | SPL Token, 261 B (legacy261): token leg — a donation to the pool vault | 21825 | 21825 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 4 | — | 2 |
| `sweep_trader` | Token-2022, 301 B, boost spent (mature): nothing to sweep | 18403 | 18403 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 2 | — | 2 |
| `flag_venue_upgraded` | PumpSwap, the dumped 10,485,760-byte ProgramData | 6036 | 6036 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | PumpSwap, the dumped ProgramData | 8506 | 8506 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
