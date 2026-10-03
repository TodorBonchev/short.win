# M6 CU bench (PumpSwap venue)

Compute units of every PumpSwap-path instruction and the `→ PumpSwap` graduation transition (ARCH §9 M6: T-CU) of the **release** short.win (`anchor build --tools-version v1.57 --arch v3`) against the **dumped mainnet** pump, pump-fees and PumpSwap programs and canonical pools (`tests/fixtures/`, groups `pamm_state`, `pamm_synthetic`, `pump_state`), measured with Mollusk 0.15.1 on account states built by the LiteSVM harness on the PumpSwap fork, with the same short.win, pump, pump-fees, SPL Token (p-token), Token-2022 and ATA ELFs; LiteSVM 0.16.0 (mainnet feature set) runs the same instruction behind a 1.4 M `SetComputeUnitLimit` as a cross-check (its own `consumed` line) and gives its instruction-trace entries, its peak stack height and the venue's own CU (its outer frames: its pump-fees, token, System and event CPIs included). Budgets are ARCH §6.1's PROPOSED transaction limits; `results.md` names the source of each. Written by `M6_CU_WRITE=1 cargo test --manifest-path tests/bench/m6_cu/Cargo.toml --test m6_cu` (README.md); the test fails when a Mollusk figure exceeds its budget, the two harnesses disagree, or a case does not execute.

| Instruction | Case | Mollusk CU | LiteSVM CU | venue CU | Budget | Budget source | Trace | ARCH §6.2 (PumpSwap) | Peak height |
|---|---|---|---|---|---|---|---|---|---|
| `create_market` | Auto, Token-2022, 301 B, boost spent (mature): PumpSwap, the init set | 134442 | 134442 | — | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, SPL Token, 261 B (legacy261): PumpSwap, the init set | 113490 | 113490 | — | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 301 B, boost active (fresh): PumpSwap, the init set | 117942 | 117942 | — | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 301 B, cashback (cashback): PumpSwap, the init set | 132940 | 132940 | — | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `open_short` | Token-2022, 301 B, boost spent (mature): first open of a wallet | 198696 | 198696 | 75663 | 250000 | §6.1 `open_short` PumpSwap | 23 | 20 (≤ 23) | 3 |
| `open_short` | SPL Token, 261 B (legacy261): first open of a wallet | 181954 | 181954 | 67533 | 250000 | §6.1 `open_short` PumpSwap | 23 | 20 (≤ 23) | 3 |
| `open_short` | SPL Token, 211 B, no creator (legacy211): first open of a wallet | 181384 | 181384 | 64026 | 250000 | §6.1 `open_short` PumpSwap | 22 | 20 (≤ 23) | 3 |
| `open_short` | Token-2022, 301 B, cashback (cashback): first open of a wallet | 201320 | 201320 | 82865 | 250000 | §6.1 `open_short` PumpSwap | 23 | 20 (≤ 23) | 3 |
| `open_short` | Token-2022, 301 B, boost spent (mature): second position of a wallet | 196163 | 196163 | 75666 | 250000 | §6.1 `open_short` PumpSwap | 22 | 20 (≤ 23) | 3 |
| `open_short` | Token-2022, 301 B, boost spent (mature): first open, normalising | 207390 | 207390 | 75663 | 250000 | §6.1 `open_short` PumpSwap | 26 | 20 (≤ 23) | 3 |
| `open_short` | Token-2022, 301 B, boost spent (mature): costliest — normalising, creator change into an empty vault, last recipients | 206140 | 206140 | 71299 | 250000 | §6.1 `open_short` PumpSwap | 26 | 20 (≤ 23) | 3 |
| `close_short` | Token-2022, 301 B, boost spent (mature) | 163771 | 163771 | 83529 | 250000 | §6.1 `close_short` curve / PumpSwap | 17 | 16 (≤ 19) | 3 |
| `close_short` | SPL Token, 261 B (legacy261) | 152303 | 152303 | 74808 | 250000 | §6.1 `close_short` curve / PumpSwap | 17 | 16 (≤ 19) | 3 |
| `close_short` | SPL Token, 211 B, no creator (legacy211) | 150253 | 150253 | 73032 | 250000 | §6.1 `close_short` curve / PumpSwap | 16 | 16 (≤ 19) | 3 |
| `close_short` | Token-2022, 301 B, cashback (cashback) | 169026 | 169026 | 87990 | 250000 | §6.1 `close_short` curve / PumpSwap | 17 | 16 (≤ 19) | 3 |
| `close_short` | Token-2022, 301 B, boost spent (mature): normalising | 172976 | 172976 | 83529 | 250000 | §6.1 `close_short` curve / PumpSwap | 20 | 16 (≤ 19) | 3 |
| `close_short` | Token-2022, 301 B, boost spent (mature): costliest — normalising, creator change into an empty vault, last recipients | 171726 | 171726 | 79165 | 250000 | §6.1 `close_short` curve / PumpSwap | 20 | 16 (≤ 19) | 3 |
| `liquidate` | Token-2022, 301 B, boost spent (mature): after a real pump (× 2.8), both legs | 185610 | 185610 | 83394 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 17 | 16 (≤ 19) | 3 |
| `expire` | Token-2022, 301 B, boost spent (mature): past the deadline, healthy | 181231 | 181231 | 83529 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 17 | 16 (≤ 19) | 3 |
| `liquidate` | SPL Token, 261 B (legacy261): after a real pump (× 2.8), both legs | 171821 | 171821 | 74807 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 17 | 16 (≤ 19) | 3 |
| `expire` | SPL Token, 261 B (legacy261): past the deadline, healthy | 169804 | 169804 | 74808 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 17 | 16 (≤ 19) | 3 |
| `liquidate` | Token-2022, 301 B, boost spent (mature): costliest — normalising, creator change, last recipients | 193451 | 193451 | 79030 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 20 | 16 (≤ 19) | 3 |
| `liquidate` | graduation path: on the migrated pool after the warm-up | 194632 | 194632 | 94100 | 300000 | §6.1 `liquidate` / `expire` PumpSwap | 17 | 16 (≤ 19) | 3 |
| `sol_settle` | Token-2022, 301 B, boost spent (mature): healthy, the no-change call | 50290 | 50290 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | Token-2022, 301 B, boost spent (mature): S3 under the buy halt (disable_flags bit 3), after s3_wait_s | 56838 | 56838 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `crank_price` | Token-2022, 301 B, boost spent (mature): roll + record (first crank of a second) | 34817 | 34817 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | SPL Token, 261 B (legacy261): roll + record (first crank of a second) | 34224 | 34224 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 301 B, boost spent (mature): record only (same second again) | 30625 | 30625 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 301 B, boost spent (mature): roll + record after a coin-creator change: the creator re-cached (M6-SEC-4) | 37929 | 37929 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | graduation transition: `PumpCurve → PumpSwap` (the pool cache, the ring rewrite, the warm-up) | 32092 | 32092 | — | 60000 | §6.1 `crank_price` | 4 | 2 (≤ 2) | 2 |
| `crank_price` | graduation transition of a market re-detected onto the curve (`None → PumpCurve`): the PumpSwap cache derived at the transition | 56185 | 56185 | — | 60000 | §6.1 `crank_price` | 4 | 2 (≤ 2) | 2 |
| `sweep_trader` | Token-2022, 301 B, boost spent (mature): token leg — a donation to the pool vault | 24484 | 24484 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 4 | — | 2 |
| `sweep_trader` | SPL Token, 261 B (legacy261): token leg — a donation to the pool vault | 21945 | 21945 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 4 | — | 2 |
| `sweep_trader` | Token-2022, 301 B, boost spent (mature): nothing to sweep | 18525 | 18525 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 2 | — | 2 |
| `flag_venue_upgraded` | PumpSwap, the dumped 10,485,760-byte ProgramData | 6028 | 6028 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | PumpSwap, the dumped ProgramData | 8440 | 8440 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
