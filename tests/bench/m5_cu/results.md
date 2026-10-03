# M5 CU bench (pump curve)

Compute units of every curve-path instruction (ARCH §9 M5: T-CU) of the **release** short.win (`anchor build --tools-version v1.57 --arch v3`) against the **dumped mainnet** pump, pump-fees and PumpSwap programs and coins (`tests/fixtures/`, group `pump_state`), measured with Mollusk 0.15.1 on account states built by the LiteSVM harness on the pump fork, with the same short.win, pump, pump-fees, SPL Token (p-token), Token-2022 and ATA ELFs; LiteSVM 0.16.0 (mainnet feature set) runs the same instruction behind a 1.4 M `SetComputeUnitLimit` as a cross-check (its own `consumed` line) and gives its instruction-trace entries, its peak stack height and pump's own CU (its outermost frames: its pump-fees, token, System and event CPIs included). Budgets are ARCH §6.1's PROPOSED transaction limits; `results.md` names the source of each. Written by `M5_CU_WRITE=1 cargo test --manifest-path tests/bench/m5_cu/Cargo.toml --test m5_cu` (README.md); the test fails when a Mollusk figure exceeds its budget, the two harnesses disagree, or a case does not execute.

| Instruction | Case | Mollusk CU | LiteSVM CU | pump CU | Budget | Budget source | Trace | ARCH §6.2 (curve) | Peak height |
|---|---|---|---|---|---|---|---|---|---|
| `create_market` | Auto, SPL Token, 115 B curve, no creator (legacy115): `PumpCurve`, the trader's base ATA and pump UVA created | 141400 | 141400 | 17004 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 115 B curve (t22_115): `PumpCurve`, the trader's base ATA and pump UVA created | 169823 | 169823 | 15504 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 125 B curve (fresh): `PumpCurve`, the trader's base ATA and pump UVA created | 143070 | 143070 | 15504 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 151 B curve near completion (near): `PumpCurve`, the trader's base ATA and pump UVA created | 169836 | 169836 | 17004 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 151 B curve near completion (near), completed by a third party: `PumpMigrating`, with the init set | 169985 | 169985 | 17004 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, PumpSwap-class coin (graduated): `PumpSwap` on its canonical pool, the pAMM init set (M6) | 138942 | 138942 | — | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, graduated elsewhere (Fartcoin): list-only `None` (detection set only) | 57949 | 57949 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `open_short` | SPL Token, 115 B curve, no creator (legacy115): first open of a wallet (position + exposure created) | 171979 | 171979 | 74791 | 250000 | §6.1 `open_short` curve | 14 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 115 B curve (t22_115): first open of a wallet (position + exposure created) | 181908 | 181908 | 80127 | 250000 | §6.1 `open_short` curve | 14 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 125 B curve (fresh): first open of a wallet (position + exposure created) | 176386 | 176386 | 69630 | 250000 | §6.1 `open_short` curve | 14 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 151 B curve near completion (near): first open of a wallet (position + exposure created) | 183340 | 183340 | 81628 | 250000 | §6.1 `open_short` curve | 14 | 12 (≤ 15) | 3 |
| `open_short` | SPL Token, 115 B curve, no creator (legacy115): second position of a wallet | 174403 | 174403 | 74794 | 250000 | §6.1 `open_short` curve | 13 | 12 (≤ 15) | 3 |
| `open_short` | SPL Token, 115 B curve, no creator (legacy115): first open, normalising (lamports to the trader, tokens to trader_base_ata) | 178332 | 178332 | 74791 | 250000 | §6.1 `open_short` curve | 17 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 115 B curve (t22_115): second position of a wallet | 180973 | 180973 | 80130 | 250000 | §6.1 `open_short` curve | 13 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 115 B curve (t22_115): first open, normalising (lamports to the trader, tokens to trader_base_ata) | 190597 | 190597 | 80127 | 250000 | §6.1 `open_short` curve | 17 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 151 B curve near completion (near): costliest — first open, normalising, after a creator change into an empty vault (top-up), last recipients | 201693 | 201693 | 85383 | 250000 | §6.1 `open_short` curve | 18 | 12 (≤ 15) | 3 |
| `close_short` | SPL Token, 115 B curve, no creator (legacy115) | 150039 | 150039 | 83790 | 250000 | §6.1 `close_short` curve / PumpSwap | 12 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 115 B curve (t22_115) | 160254 | 160254 | 90949 | 250000 | §6.1 `close_short` curve / PumpSwap | 13 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 125 B curve (fresh) | 149984 | 149984 | 80449 | 250000 | §6.1 `close_short` curve / PumpSwap | 13 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 151 B curve near completion (near) | 162042 | 162042 | 92447 | 250000 | §6.1 `close_short` curve / PumpSwap | 13 | 12 (≤ 15) | 3 |
| `close_short` | SPL Token, 115 B curve, no creator (legacy115): normalising | 156903 | 156903 | 83790 | 250000 | §6.1 `close_short` curve / PumpSwap | 15 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 115 B curve (t22_115): normalising | 169454 | 169454 | 90949 | 250000 | §6.1 `close_short` curve / PumpSwap | 16 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 115 B curve (t22_115): costliest — normalising, after a creator change into an empty vault (top-up in B), last recipients | 177846 | 177846 | 93204 | 250000 | §6.1 `close_short` curve / PumpSwap | 17 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 151 B curve near completion (near): costliest — normalising, after a creator change into an empty vault (top-up in B), last recipients | 181134 | 181134 | 96202 | 250000 | §6.1 `close_short` curve / PumpSwap | 17 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 151 B curve near completion (near): completing buyback (q = rT; `PumpCurve → PumpMigrating` in the same instruction) | 174807 | 174807 | 97889 | 250000 | §6.1 `close_short` curve / PumpSwap | 15 | 12 (≤ 15) | 3 |
| `liquidate` | SPL Token, 115 B curve, no creator (legacy115): after a real pump (40 % of vT bought), both legs | 171030 | 171030 | 83788 | 300000 | §6.1 `liquidate` / `expire` curve | 12 | 12 (≤ 15) | 3 |
| `liquidate` | Token-2022, 115 B curve (t22_115): after a real pump (40 % of vT bought), both legs | 180909 | 180909 | 90948 | 300000 | §6.1 `liquidate` / `expire` curve | 13 | 12 (≤ 15) | 3 |
| `liquidate` | Token-2022, 125 B curve (fresh): after a real pump (40 % of vT bought), both legs | 171451 | 171451 | 80449 | 300000 | §6.1 `liquidate` / `expire` curve | 13 | 12 (≤ 15) | 3 |
| `liquidate` | Token-2022, 115 B curve (t22_115): costliest — normalising, creator change into an empty vault, last recipients | 198387 | 198387 | 93203 | 300000 | §6.1 `liquidate` / `expire` curve | 17 | 12 (≤ 15) | 3 |
| `expire` | SPL Token, 115 B curve, no creator (legacy115): past the deadline, healthy | 167394 | 167394 | 83790 | 300000 | §6.1 `liquidate` / `expire` curve | 12 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 115 B curve (t22_115): past the deadline, healthy | 177625 | 177625 | 90949 | 300000 | §6.1 `liquidate` / `expire` curve | 13 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 125 B curve (fresh): past the deadline, healthy | 167824 | 167824 | 80449 | 300000 | §6.1 `liquidate` / `expire` curve | 13 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 151 B curve near completion (near): past the deadline, healthy | 179492 | 179492 | 92447 | 300000 | §6.1 `liquidate` / `expire` curve | 13 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 151 B curve near completion (near): costliest — normalising, creator change into an empty vault, last recipients | 198470 | 198470 | 96202 | 300000 | §6.1 `liquidate` / `expire` curve | 17 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 151 B curve near completion (near): completing buyback (q = rT; `PumpCurve → PumpMigrating`) | 189050 | 189050 | 97889 | 300000 | §6.1 `liquidate` / `expire` curve | 15 | 12 (≤ 15) | 3 |
| `sol_settle` | SPL Token, 115 B curve, no creator (legacy115): healthy, the no-change call | 42383 | 42383 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | Token-2022, 115 B curve (t22_115): healthy, the no-change call | 42772 | 42772 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | `PumpMigrating` (stuck migration): S3 at migration_seen_at + 70 min, FV on the post-migration pool | 64984 | 64984 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `sol_settle` | `None` (graduated elsewhere; the detection set as ReadSet): S3 after s3_wait_s (M2-SEC-5) | 45577 | 45577 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `crank_price` | SPL Token, 115 B curve, no creator (legacy115): roll + record (first crank of a second) | 31544 | 31544 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 115 B curve (t22_115): roll + record (first crank of a second) | 31572 | 31572 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 125 B curve (fresh): roll + record (first crank of a second) | 31808 | 31808 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 151 B curve near completion (near): roll + record (first crank of a second) | 31802 | 31802 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 115 B curve (t22_115): record only (same second again) | 26878 | 26878 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 151 B curve near completion (near): `PumpCurve → PumpMigrating` (completed by a third party), chains start | 36912 | 36912 | — | 60000 | §6.1 `crank_price` | 4 | 2 (≤ 2) | 2 |
| `crank_price` | `PumpMigrating → None` (migrated elsewhere; `graduated_elsewhere`) | 26355 | 26355 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | `None` (graduated elsewhere), the detection set, the S3 chain running | 16770 | 16770 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 115 B curve (t22_115): `None → PumpCurve` re-detection (curve cache, price start, Tier D, chains and fp_* cleared) | 50836 | 50836 | — | 60000 | §6.1 `crank_price` | 4 | 2 (≤ 2) | 2 |
| `sweep_trader` | SPL Token, 115 B curve, no creator (legacy115): token leg — lamports on the trader, tokens on trader_base_ata | 22114 | 22114 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 4 | — | 2 |
| `sweep_trader` | Token-2022, 115 B curve (t22_115): token leg — lamports on the trader, tokens on trader_base_ata | 24653 | 24653 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 4 | — | 2 |
| `sweep_trader` | Token-2022, 115 B curve (t22_115): nothing to sweep | 18694 | 18694 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 2 | — | 2 |
| `flag_venue_upgraded` | pump, the dumped 10,485,760-byte ProgramData | 6028 | 6028 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | pump-fees, the dumped 948,989-byte ProgramData | 6028 | 6028 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | PumpSwap, the dumped 10,485,760-byte ProgramData | 6028 | 6028 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | pump, the dumped ProgramData | 8440 | 8440 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | pump-fees, the dumped ProgramData | 8440 | 8440 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
