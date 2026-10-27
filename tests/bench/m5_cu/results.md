# M5 CU bench (pump curve)

Compute units of every curve-path instruction (ARCH §9 M5: T-CU) of the **release** short.win (`anchor build --tools-version v1.57 --arch v3`) against the **dumped mainnet** pump, pump-fees and PumpSwap programs and coins (`tests/fixtures/`, group `pump_state`), measured with Mollusk 0.15.1 on account states built by the LiteSVM harness on the pump fork, with the same short.win, pump, pump-fees, SPL Token (p-token), Token-2022 and ATA ELFs; LiteSVM 0.16.0 (mainnet feature set) runs the same instruction behind a 1.4 M `SetComputeUnitLimit` as a cross-check (its own `consumed` line) and gives its instruction-trace entries, its peak stack height and pump's own CU (its outermost frames: its pump-fees, token, System and event CPIs included). Budgets are ARCH §6.1's PROPOSED transaction limits; `results.md` names the source of each. Written by `M5_CU_WRITE=1 cargo test --manifest-path tests/bench/m5_cu/Cargo.toml --test m5_cu` (README.md); the test fails when a Mollusk figure exceeds its budget, the two harnesses disagree, or a case does not execute.

| Instruction | Case | Mollusk CU | LiteSVM CU | pump CU | Budget | Budget source | Trace | ARCH §6.2 (curve) | Peak height |
|---|---|---|---|---|---|---|---|---|---|
| `create_market` | Auto, SPL Token, 115 B curve, no creator (legacy115): `PumpCurve`, the trader's base ATA and pump UVA created | 141531 | 141531 | 17046 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 115 B curve (t22_115): `PumpCurve`, the trader's base ATA and pump UVA created | 171835 | 171835 | 15546 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 125 B curve (fresh): `PumpCurve`, the trader's base ATA and pump UVA created | 145148 | 145148 | 15546 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 151 B curve near completion (near): `PumpCurve`, the trader's base ATA and pump UVA created | 172054 | 172054 | 17046 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 151 B curve near completion (near), completed by a third party: `PumpMigrating`, with the init set | 172248 | 172248 | 17046 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, PumpSwap-class coin (graduated): `PumpSwap` on its canonical pool, the pAMM init set (M6) | 141125 | 141125 | — | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, graduated elsewhere (Fartcoin): list-only `None` (detection set only) | 58114 | 58114 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `open_short` | SPL Token, 115 B curve, no creator (legacy115): first open of a wallet (position + exposure created) | 139385 | 139385 | 46909 | 250000 | §6.1 `open_short` curve | 13 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 115 B curve (t22_115): first open of a wallet (position + exposure created) | 146436 | 146436 | 48934 | 250000 | §6.1 `open_short` curve | 13 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 125 B curve (fresh): first open of a wallet (position + exposure created) | 145664 | 145664 | 43067 | 250000 | §6.1 `open_short` curve | 13 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 151 B curve near completion (near): first open of a wallet (position + exposure created) | 147038 | 147038 | 49511 | 250000 | §6.1 `open_short` curve | 14 | 12 (≤ 15) | 3 |
| `open_short` | SPL Token, 115 B curve, no creator (legacy115): second position of a wallet | 142105 | 142105 | 47042 | 250000 | §6.1 `open_short` curve | 12 | 12 (≤ 15) | 3 |
| `open_short` | SPL Token, 115 B curve, no creator (legacy115): first open, normalising (lamports to the trader, tokens to trader_base_ata) | 145746 | 145746 | 46909 | 250000 | §6.1 `open_short` curve | 16 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 115 B curve (t22_115): second position of a wallet | 145796 | 145796 | 49067 | 250000 | §6.1 `open_short` curve | 12 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 115 B curve (t22_115): first open, normalising (lamports to the trader, tokens to trader_base_ata) | 155548 | 155548 | 48934 | 250000 | §6.1 `open_short` curve | 16 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 151 B curve near completion (near): costliest — first open, normalising, after a creator change into an empty vault, the last buyback recipient | 156119 | 156119 | 49693 | 250000 | §6.1 `open_short` curve | 17 | 12 (≤ 15) | 3 |
| `close_short` | SPL Token, 115 B curve, no creator (legacy115) | 109801 | 109801 | 49600 | 250000 | §6.1 `close_short` curve / PumpSwap | 9 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 115 B curve (t22_115) | 115307 | 115307 | 51625 | 250000 | §6.1 `close_short` curve / PumpSwap | 9 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 125 B curve (fresh) | 109537 | 109537 | 45625 | 250000 | §6.1 `close_short` curve / PumpSwap | 9 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 151 B curve near completion (near) | 114095 | 114095 | 50124 | 250000 | §6.1 `close_short` curve / PumpSwap | 9 | 12 (≤ 15) | 3 |
| `close_short` | SPL Token, 115 B curve, no creator (legacy115): normalising | 116599 | 116599 | 49600 | 250000 | §6.1 `close_short` curve / PumpSwap | 12 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 115 B curve (t22_115): normalising | 124856 | 124856 | 51625 | 250000 | §6.1 `close_short` curve / PumpSwap | 12 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 115 B curve (t22_115): costliest — normalising, after a creator change into an empty vault, the last buyback recipient | 125052 | 125052 | 51807 | 250000 | §6.1 `close_short` curve / PumpSwap | 12 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 151 B curve near completion (near): costliest — normalising, after a creator change into an empty vault, the last buyback recipient | 123840 | 123840 | 50306 | 250000 | §6.1 `close_short` curve / PumpSwap | 12 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 151 B curve near completion (near): completing buyback (q = rT; `PumpCurve → PumpMigrating` in the same instruction) | 126819 | 126819 | 55564 | 250000 | §6.1 `close_short` curve / PumpSwap | 11 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 151 B curve near completion (near): synthetic past rT (the crowd left rT = q/2: `buy_v3(q)` takes rT on the curve and q − rT from the would-be pool; `PumpCurve → PumpMigrating`) | 137431 | 137431 | 64578 | 250000 | §6.1 `close_short` curve / PumpSwap | 12 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 151 B curve near completion (near): synthetic past rT (the crowd left rT = q/2: `buy_v3(q)` takes rT on the curve and q − rT from the would-be pool; `PumpCurve → PumpMigrating`), worst ring (every ring entry past rT) | 137429 | 137429 | 64578 | 250000 | §6.1 `close_short` curve / PumpSwap | 12 | 12 (≤ 15) | 3 |
| `liquidate` | SPL Token, 115 B curve, no creator (legacy115): after a real pump (40 % of vT bought), both legs | 131137 | 131137 | 49598 | 300000 | §6.1 `liquidate` / `expire` curve | 9 | 12 (≤ 15) | 3 |
| `liquidate` | Token-2022, 115 B curve (t22_115): after a real pump (40 % of vT bought), both legs | 136308 | 136308 | 51624 | 300000 | §6.1 `liquidate` / `expire` curve | 9 | 12 (≤ 15) | 3 |
| `liquidate` | Token-2022, 125 B curve (fresh): after a real pump (40 % of vT bought), both legs | 131352 | 131352 | 45625 | 300000 | §6.1 `liquidate` / `expire` curve | 9 | 12 (≤ 15) | 3 |
| `liquidate` | Token-2022, 115 B curve (t22_115): costliest — normalising, creator change into an empty vault, last recipients | 145930 | 145930 | 51806 | 300000 | §6.1 `liquidate` / `expire` curve | 12 | 12 (≤ 15) | 3 |
| `liquidate` | ECON's D@80 % (fresh, Token-2022): synthetic past rT (the crowd left rT = q/2, the price × 2.4), both legs | 153163 | 153163 | 60077 | 300000 | §6.1 `liquidate` / `expire` curve | 12 | 12 (≤ 15) | 3 |
| `liquidate` | ECON's D@80 % (fresh, Token-2022): synthetic past rT (the crowd left rT = q/2, the price × 2.4), both legs, worst ring (every ring entry past rT) | 160887 | 160887 | 60077 | 300000 | §6.1 `liquidate` / `expire` curve | 12 | 12 (≤ 15) | 3 |
| `expire` | SPL Token, 115 B curve, no creator (legacy115): past the deadline, healthy | 127543 | 127543 | 49600 | 300000 | §6.1 `liquidate` / `expire` curve | 9 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 115 B curve (t22_115): past the deadline, healthy | 133065 | 133065 | 51625 | 300000 | §6.1 `liquidate` / `expire` curve | 9 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 125 B curve (fresh): past the deadline, healthy | 127764 | 127764 | 45625 | 300000 | §6.1 `liquidate` / `expire` curve | 9 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 151 B curve near completion (near): past the deadline, healthy | 131932 | 131932 | 50124 | 300000 | §6.1 `liquidate` / `expire` curve | 9 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 151 B curve near completion (near): costliest — normalising, creator change into an empty vault, last recipients | 141554 | 141554 | 50306 | 300000 | §6.1 `liquidate` / `expire` curve | 12 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 151 B curve near completion (near): completing buyback (q = rT; `PumpCurve → PumpMigrating`) | 141265 | 141265 | 55564 | 300000 | §6.1 `liquidate` / `expire` curve | 11 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 151 B curve near completion (near): synthetic past rT (the crowd left rT = q/2 after the deadline; `PumpCurve → PumpMigrating`) | 156725 | 156725 | 64578 | 300000 | §6.1 `liquidate` / `expire` curve | 12 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 151 B curve near completion (near): synthetic past rT (the crowd left rT = q/2 after the deadline; `PumpCurve → PumpMigrating`), worst ring (every ring entry past rT) | 158286 | 158286 | 64578 | 300000 | §6.1 `liquidate` / `expire` curve | 12 | 12 (≤ 15) | 3 |
| `sol_settle` | SPL Token, 115 B curve, no creator (legacy115): healthy, the no-change call | 43721 | 43721 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | Token-2022, 115 B curve (t22_115): healthy, the no-change call | 44117 | 44117 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | `PumpMigrating` (stuck migration): S3 at migration_seen_at + 70 min, FV on the post-migration pool | 64833 | 64833 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `sol_settle` | `PumpMigrating` after a third party's synthetic completion (`pc ≠ 0`): S3 at migration_seen_at + 70 min, FV ρ-scaled on the pool migrate_v2 will create | 69355 | 69355 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `sol_settle` | `None` (graduated elsewhere; the detection set as ReadSet): S3 after s3_wait_s (M2-SEC-5) | 44941 | 44941 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `crank_price` | SPL Token, 115 B curve, no creator (legacy115): roll + record (first crank of a second) | 32408 | 32408 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 115 B curve (t22_115): roll + record (first crank of a second) | 32442 | 32442 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 125 B curve (fresh): roll + record (first crank of a second) | 32672 | 32672 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 151 B curve near completion (near): roll + record (first crank of a second) | 32670 | 32670 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 115 B curve (t22_115): record only (same second again) | 27787 | 27787 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 151 B curve near completion (near): `PumpCurve → PumpMigrating` (completed by a third party), chains start | 37062 | 37062 | — | 60000 | §6.1 `crank_price` | 4 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 151 B curve near completion (near): `PumpCurve → PumpMigrating` after a third party's synthetic completion (`buy_v3` past rT, `pc ≠ 0`), chains start | 37074 | 37074 | — | 60000 | §6.1 `crank_price` | 4 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 151 B curve near completion (near): `PumpMigrating` after short.win's own synthetic close (`pc ≠ 0`), roll + record | 35234 | 35234 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | `PumpMigrating → None` (migrated elsewhere; `graduated_elsewhere`) | 26397 | 26397 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | `None` (graduated elsewhere), the detection set, the S3 chain running | 16777 | 16777 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 115 B curve (t22_115): `None → PumpCurve` re-detection (curve cache, price start, Tier D, chains and fp_* cleared) | 50757 | 50757 | — | 60000 | §6.1 `crank_price` | 4 | 2 (≤ 2) | 2 |
| `sweep_trader` | SPL Token, 115 B curve, no creator (legacy115): token leg — lamports on the trader, tokens on trader_base_ata | 21996 | 21996 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 4 | — | 2 |
| `sweep_trader` | Token-2022, 115 B curve (t22_115): token leg — lamports on the trader, tokens on trader_base_ata | 24951 | 24951 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 4 | — | 2 |
| `sweep_trader` | Token-2022, 115 B curve (t22_115): nothing to sweep | 18574 | 18574 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 2 | — | 2 |
| `flag_venue_upgraded` | pump, the dumped 10,485,760-byte ProgramData | 6036 | 6036 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | pump-fees, the dumped 948,989-byte ProgramData | 6046 | 6046 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | PumpSwap, the dumped 10,485,760-byte ProgramData | 6036 | 6036 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | pump, the dumped ProgramData | 8506 | 8506 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | pump-fees, the dumped ProgramData | 8527 | 8527 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
