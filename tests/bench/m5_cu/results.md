# M5 CU bench (pump curve)

Compute units of every curve-path instruction (ARCH §9 M5: T-CU) of the **release** short.win (`anchor build --tools-version v1.57 --arch v3`) against the **dumped mainnet** pump, pump-fees and PumpSwap programs and coins (`tests/fixtures/`, group `pump_state`), measured with Mollusk 0.15.1 on account states built by the LiteSVM harness on the pump fork, with the same short.win, pump, pump-fees, SPL Token (p-token), Token-2022 and ATA ELFs; LiteSVM 0.16.0 (mainnet feature set) runs the same instruction behind a 1.4 M `SetComputeUnitLimit` as a cross-check (its own `consumed` line) and gives its instruction-trace entries, its peak stack height and pump's own CU (its outermost frames: its pump-fees, token, System and event CPIs included). Budgets are ARCH §6.1's PROPOSED transaction limits; `results.md` names the source of each. Written by `M5_CU_WRITE=1 cargo test --manifest-path tests/bench/m5_cu/Cargo.toml --test m5_cu` (README.md); the test fails when a Mollusk figure exceeds its budget, the two harnesses disagree, or a case does not execute.

| Instruction | Case | Mollusk CU | LiteSVM CU | pump CU | Budget | Budget source | Trace | ARCH §6.2 (curve) | Peak height |
|---|---|---|---|---|---|---|---|---|---|
| `create_market` | Auto, SPL Token, 115 B curve, no creator (legacy115): `PumpCurve`, the trader's base ATA and pump UVA created | 121247 | 121247 | 17004 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 115 B curve (t22_115): `PumpCurve`, the trader's base ATA and pump UVA created | 137660 | 137660 | 15504 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 125 B curve (fresh): `PumpCurve`, the trader's base ATA and pump UVA created | 107907 | 107907 | 15504 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 151 B curve near completion (near): `PumpCurve`, the trader's base ATA and pump UVA created | 134673 | 134673 | 17004 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 151 B curve near completion (near), completed by a third party: `PumpMigrating`, with the init set | 134818 | 134818 | 17004 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, PumpSwap-class coin (graduated): list-only `None` (detection set only) | 62240 | 62240 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | Auto, graduated elsewhere (Fartcoin): list-only `None` (detection set only) | 57946 | 57946 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `open_short` | SPL Token, 115 B curve, no creator (legacy115): first open of a wallet (position + exposure created) | 170400 | 170400 | 74791 | 250000 | §6.1 `open_short` curve | 14 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 115 B curve (t22_115): first open of a wallet (position + exposure created) | 180290 | 180290 | 80112 | 250000 | §6.1 `open_short` curve | 14 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 125 B curve (fresh): first open of a wallet (position + exposure created) | 174767 | 174767 | 69615 | 250000 | §6.1 `open_short` curve | 14 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 151 B curve near completion (near): first open of a wallet (position + exposure created) | 181609 | 181609 | 81613 | 250000 | §6.1 `open_short` curve | 14 | 12 (≤ 15) | 3 |
| `open_short` | SPL Token, 115 B curve, no creator (legacy115): second position of a wallet | 172821 | 172821 | 74794 | 250000 | §6.1 `open_short` curve | 13 | 12 (≤ 15) | 3 |
| `open_short` | SPL Token, 115 B curve, no creator (legacy115): first open, normalising (lamports to the trader, tokens to trader_base_ata) | 176751 | 176751 | 74791 | 250000 | §6.1 `open_short` curve | 17 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 115 B curve (t22_115): second position of a wallet | 179239 | 179239 | 80115 | 250000 | §6.1 `open_short` curve | 13 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 115 B curve (t22_115): first open, normalising (lamports to the trader, tokens to trader_base_ata) | 188962 | 188962 | 80112 | 250000 | §6.1 `open_short` curve | 17 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 151 B curve near completion (near): costliest — first open, normalising, after a creator change into an empty vault (top-up), last recipients | 199945 | 199945 | 85368 | 250000 | §6.1 `open_short` curve | 18 | 12 (≤ 15) | 3 |
| `close_short` | SPL Token, 115 B curve, no creator (legacy115) | 150574 | 150574 | 83790 | 250000 | §6.1 `close_short` curve / PumpSwap | 12 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 115 B curve (t22_115) | 160759 | 160759 | 90934 | 250000 | §6.1 `close_short` curve / PumpSwap | 13 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 125 B curve (fresh) | 150489 | 150489 | 80434 | 250000 | §6.1 `close_short` curve / PumpSwap | 13 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 151 B curve near completion (near) | 162367 | 162367 | 92432 | 250000 | §6.1 `close_short` curve / PumpSwap | 13 | 12 (≤ 15) | 3 |
| `close_short` | SPL Token, 115 B curve, no creator (legacy115): normalising | 156924 | 156924 | 83790 | 250000 | §6.1 `close_short` curve / PumpSwap | 15 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 115 B curve (t22_115): normalising | 169430 | 169430 | 90934 | 250000 | §6.1 `close_short` curve / PumpSwap | 16 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 115 B curve (t22_115): costliest — normalising, after a creator change into an empty vault (top-up in B), last recipients | 177822 | 177822 | 93189 | 250000 | §6.1 `close_short` curve / PumpSwap | 17 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 151 B curve near completion (near): costliest — normalising, after a creator change into an empty vault (top-up in B), last recipients | 180930 | 180930 | 96187 | 250000 | §6.1 `close_short` curve / PumpSwap | 17 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 151 B curve near completion (near): completing buyback (q = rT; `PumpCurve → PumpMigrating` in the same instruction) | 175135 | 175135 | 97874 | 250000 | §6.1 `close_short` curve / PumpSwap | 15 | 12 (≤ 15) | 3 |
| `liquidate` | SPL Token, 115 B curve, no creator (legacy115): after a real pump (40 % of vT bought), both legs | 171345 | 171345 | 83788 | 300000 | §6.1 `liquidate` / `expire` curve | 12 | 12 (≤ 15) | 3 |
| `liquidate` | Token-2022, 115 B curve (t22_115): after a real pump (40 % of vT bought), both legs | 181192 | 181192 | 90933 | 300000 | §6.1 `liquidate` / `expire` curve | 13 | 12 (≤ 15) | 3 |
| `liquidate` | Token-2022, 125 B curve (fresh): after a real pump (40 % of vT bought), both legs | 171734 | 171734 | 80434 | 300000 | §6.1 `liquidate` / `expire` curve | 13 | 12 (≤ 15) | 3 |
| `liquidate` | Token-2022, 115 B curve (t22_115): costliest — normalising, creator change into an empty vault, last recipients | 198653 | 198653 | 93188 | 300000 | §6.1 `liquidate` / `expire` curve | 17 | 12 (≤ 15) | 3 |
| `expire` | SPL Token, 115 B curve, no creator (legacy115): past the deadline, healthy | 167115 | 167115 | 83790 | 300000 | §6.1 `liquidate` / `expire` curve | 12 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 115 B curve (t22_115): past the deadline, healthy | 177727 | 177727 | 90934 | 300000 | §6.1 `liquidate` / `expire` curve | 13 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 125 B curve (fresh): past the deadline, healthy | 167513 | 167513 | 80434 | 300000 | §6.1 `liquidate` / `expire` curve | 13 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 151 B curve near completion (near): past the deadline, healthy | 179181 | 179181 | 92432 | 300000 | §6.1 `liquidate` / `expire` curve | 13 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 151 B curve near completion (near): costliest — normalising, creator change into an empty vault, last recipients | 198142 | 198142 | 96187 | 300000 | §6.1 `liquidate` / `expire` curve | 17 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 151 B curve near completion (near): completing buyback (q = rT; `PumpCurve → PumpMigrating`) | 188737 | 188737 | 97874 | 300000 | §6.1 `liquidate` / `expire` curve | 15 | 12 (≤ 15) | 3 |
| `sol_settle` | SPL Token, 115 B curve, no creator (legacy115): healthy, the no-change call | 42122 | 42122 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | Token-2022, 115 B curve (t22_115): healthy, the no-change call | 42509 | 42509 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | `PumpMigrating` (stuck migration): S3 at migration_seen_at + 70 min, FV on the post-migration pool | 64701 | 64701 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `sol_settle` | `None` (graduated elsewhere; the detection set as ReadSet): S3 after s3_wait_s (M2-SEC-5) | 45401 | 45401 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `crank_price` | SPL Token, 115 B curve, no creator (legacy115): roll + record (first crank of a second) | 31280 | 31280 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 115 B curve (t22_115): roll + record (first crank of a second) | 31306 | 31306 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 125 B curve (fresh): roll + record (first crank of a second) | 31542 | 31542 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 151 B curve near completion (near): roll + record (first crank of a second) | 31536 | 31536 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 115 B curve (t22_115): record only (same second again) | 26611 | 26611 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 151 B curve near completion (near): `PumpCurve → PumpMigrating` (completed by a third party), chains start | 36639 | 36639 | — | 60000 | §6.1 `crank_price` | 4 | 2 (≤ 2) | 2 |
| `crank_price` | `PumpMigrating → None` (migrated elsewhere; `graduated_elsewhere`) | 26097 | 26097 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | `None` (graduated elsewhere), the detection set, the S3 chain running | 16612 | 16612 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 115 B curve (t22_115): `None → PumpCurve` re-detection (curve cache, price start, Tier D, chains and fp_* cleared) | 50676 | 50676 | — | 60000 | §6.1 `crank_price` | 4 | 2 (≤ 2) | 2 |
| `sweep_trader` | SPL Token, 115 B curve, no creator (legacy115): token leg — lamports on the trader, tokens on trader_base_ata | 22070 | 22070 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 4 | — | 2 |
| `sweep_trader` | Token-2022, 115 B curve (t22_115): token leg — lamports on the trader, tokens on trader_base_ata | 24594 | 24594 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 4 | — | 2 |
| `sweep_trader` | Token-2022, 115 B curve (t22_115): nothing to sweep | 18650 | 18650 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 2 | — | 2 |
| `flag_venue_upgraded` | pump, the dumped 10,485,760-byte ProgramData | 6026 | 6026 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | pump-fees, the dumped 948,989-byte ProgramData | 6026 | 6026 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | PumpSwap, the dumped 10,485,760-byte ProgramData | 6026 | 6026 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | pump, the dumped ProgramData | 8438 | 8438 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | pump-fees, the dumped ProgramData | 8438 | 8438 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
