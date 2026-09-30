# M5 CU bench (pump curve)

Compute units of every curve-path instruction (ARCH §9 M5: T-CU) of the **release** short.win (`anchor build --tools-version v1.57 --arch v3`) against the **dumped mainnet** pump, pump-fees and PumpSwap programs and coins (`tests/fixtures/`, group `pump_state`), measured with Mollusk 0.15.1 on account states built by the LiteSVM harness on the pump fork, with the same short.win, pump, pump-fees, SPL Token (p-token), Token-2022 and ATA ELFs; LiteSVM 0.16.0 (mainnet feature set) runs the same instruction behind a 1.4 M `SetComputeUnitLimit` as a cross-check (its own `consumed` line) and gives its instruction-trace entries, its peak stack height and pump's own CU (its outermost frames: its pump-fees, token, System and event CPIs included). Budgets are ARCH §6.1's PROPOSED transaction limits; `results.md` names the source of each. Written by `M5_CU_WRITE=1 cargo test --manifest-path tests/bench/m5_cu/Cargo.toml --test m5_cu` (README.md); the test fails when a Mollusk figure exceeds its budget, the two harnesses disagree, or a case does not execute.

| Instruction | Case | Mollusk CU | LiteSVM CU | pump CU | Budget | Budget source | Trace | ARCH §6.2 (curve) | Peak height |
|---|---|---|---|---|---|---|---|---|---|
| `create_market` | Auto, SPL Token, 115 B curve, no creator (legacy115): `PumpCurve`, the trader's base ATA and pump UVA created | 141398 | 141398 | 17004 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 115 B curve (t22_115): `PumpCurve`, the trader's base ATA and pump UVA created | 169793 | 169793 | 15504 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 125 B curve (fresh): `PumpCurve`, the trader's base ATA and pump UVA created | 143040 | 143040 | 15504 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 151 B curve near completion (near): `PumpCurve`, the trader's base ATA and pump UVA created | 169806 | 169806 | 17004 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, Token-2022, 151 B curve near completion (near), completed by a third party: `PumpMigrating`, with the init set | 169955 | 169955 | 17004 | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, PumpSwap-class coin (graduated): `PumpSwap` on its canonical pool, the pAMM init set (M6) | 138912 | 138912 | — | 250000 | §6.1 `create_market` | 15 | — | 3 |
| `create_market` | Auto, graduated elsewhere (Fartcoin): list-only `None` (detection set only) | 57947 | 57947 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `open_short` | SPL Token, 115 B curve, no creator (legacy115): first open of a wallet (position + exposure created) | 170662 | 170662 | 74791 | 250000 | §6.1 `open_short` curve | 14 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 115 B curve (t22_115): first open of a wallet (position + exposure created) | 180552 | 180552 | 80112 | 250000 | §6.1 `open_short` curve | 14 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 125 B curve (fresh): first open of a wallet (position + exposure created) | 175029 | 175029 | 69615 | 250000 | §6.1 `open_short` curve | 14 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 151 B curve near completion (near): first open of a wallet (position + exposure created) | 181871 | 181871 | 81613 | 250000 | §6.1 `open_short` curve | 14 | 12 (≤ 15) | 3 |
| `open_short` | SPL Token, 115 B curve, no creator (legacy115): second position of a wallet | 173083 | 173083 | 74794 | 250000 | §6.1 `open_short` curve | 13 | 12 (≤ 15) | 3 |
| `open_short` | SPL Token, 115 B curve, no creator (legacy115): first open, normalising (lamports to the trader, tokens to trader_base_ata) | 177013 | 177013 | 74791 | 250000 | §6.1 `open_short` curve | 17 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 115 B curve (t22_115): second position of a wallet | 179501 | 179501 | 80115 | 250000 | §6.1 `open_short` curve | 13 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 115 B curve (t22_115): first open, normalising (lamports to the trader, tokens to trader_base_ata) | 189224 | 189224 | 80112 | 250000 | §6.1 `open_short` curve | 17 | 12 (≤ 15) | 3 |
| `open_short` | Token-2022, 151 B curve near completion (near): costliest — first open, normalising, after a creator change into an empty vault (top-up), last recipients | 200207 | 200207 | 85368 | 250000 | §6.1 `open_short` curve | 18 | 12 (≤ 15) | 3 |
| `close_short` | SPL Token, 115 B curve, no creator (legacy115) | 150713 | 150713 | 83790 | 250000 | §6.1 `close_short` curve / PumpSwap | 12 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 115 B curve (t22_115) | 160898 | 160898 | 90934 | 250000 | §6.1 `close_short` curve / PumpSwap | 13 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 125 B curve (fresh) | 150628 | 150628 | 80434 | 250000 | §6.1 `close_short` curve / PumpSwap | 13 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 151 B curve near completion (near) | 162506 | 162506 | 92432 | 250000 | §6.1 `close_short` curve / PumpSwap | 13 | 12 (≤ 15) | 3 |
| `close_short` | SPL Token, 115 B curve, no creator (legacy115): normalising | 157063 | 157063 | 83790 | 250000 | §6.1 `close_short` curve / PumpSwap | 15 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 115 B curve (t22_115): normalising | 169569 | 169569 | 90934 | 250000 | §6.1 `close_short` curve / PumpSwap | 16 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 115 B curve (t22_115): costliest — normalising, after a creator change into an empty vault (top-up in B), last recipients | 177961 | 177961 | 93189 | 250000 | §6.1 `close_short` curve / PumpSwap | 17 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 151 B curve near completion (near): costliest — normalising, after a creator change into an empty vault (top-up in B), last recipients | 181069 | 181069 | 96187 | 250000 | §6.1 `close_short` curve / PumpSwap | 17 | 12 (≤ 15) | 3 |
| `close_short` | Token-2022, 151 B curve near completion (near): completing buyback (q = rT; `PumpCurve → PumpMigrating` in the same instruction) | 175269 | 175269 | 97874 | 250000 | §6.1 `close_short` curve / PumpSwap | 15 | 12 (≤ 15) | 3 |
| `liquidate` | SPL Token, 115 B curve, no creator (legacy115): after a real pump (40 % of vT bought), both legs | 171573 | 171573 | 83788 | 300000 | §6.1 `liquidate` / `expire` curve | 12 | 12 (≤ 15) | 3 |
| `liquidate` | Token-2022, 115 B curve (t22_115): after a real pump (40 % of vT bought), both legs | 181422 | 181422 | 90933 | 300000 | §6.1 `liquidate` / `expire` curve | 13 | 12 (≤ 15) | 3 |
| `liquidate` | Token-2022, 125 B curve (fresh): after a real pump (40 % of vT bought), both legs | 171964 | 171964 | 80434 | 300000 | §6.1 `liquidate` / `expire` curve | 13 | 12 (≤ 15) | 3 |
| `liquidate` | Token-2022, 115 B curve (t22_115): costliest — normalising, creator change into an empty vault, last recipients | 198883 | 198883 | 93188 | 300000 | §6.1 `liquidate` / `expire` curve | 17 | 12 (≤ 15) | 3 |
| `expire` | SPL Token, 115 B curve, no creator (legacy115): past the deadline, healthy | 167343 | 167343 | 83790 | 300000 | §6.1 `liquidate` / `expire` curve | 12 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 115 B curve (t22_115): past the deadline, healthy | 177957 | 177957 | 90934 | 300000 | §6.1 `liquidate` / `expire` curve | 13 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 125 B curve (fresh): past the deadline, healthy | 167743 | 167743 | 80434 | 300000 | §6.1 `liquidate` / `expire` curve | 13 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 151 B curve near completion (near): past the deadline, healthy | 179411 | 179411 | 92432 | 300000 | §6.1 `liquidate` / `expire` curve | 13 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 151 B curve near completion (near): costliest — normalising, creator change into an empty vault, last recipients | 198372 | 198372 | 96187 | 300000 | §6.1 `liquidate` / `expire` curve | 17 | 12 (≤ 15) | 3 |
| `expire` | Token-2022, 151 B curve near completion (near): completing buyback (q = rT; `PumpCurve → PumpMigrating`) | 188967 | 188967 | 97874 | 300000 | §6.1 `liquidate` / `expire` curve | 15 | 12 (≤ 15) | 3 |
| `sol_settle` | SPL Token, 115 B curve, no creator (legacy115): healthy, the no-change call | 42370 | 42370 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | Token-2022, 115 B curve (t22_115): healthy, the no-change call | 42759 | 42759 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | `PumpMigrating` (stuck migration): S3 at migration_seen_at + 70 min, FV on the post-migration pool | 64946 | 64946 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `sol_settle` | `None` (graduated elsewhere; the detection set as ReadSet): S3 after s3_wait_s (M2-SEC-5) | 45550 | 45550 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `crank_price` | SPL Token, 115 B curve, no creator (legacy115): roll + record (first crank of a second) | 31525 | 31525 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 115 B curve (t22_115): roll + record (first crank of a second) | 31553 | 31553 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 125 B curve (fresh): roll + record (first crank of a second) | 31789 | 31789 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 151 B curve near completion (near): roll + record (first crank of a second) | 31783 | 31783 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 115 B curve (t22_115): record only (same second again) | 26859 | 26859 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 151 B curve near completion (near): `PumpCurve → PumpMigrating` (completed by a third party), chains start | 36889 | 36889 | — | 60000 | §6.1 `crank_price` | 4 | 2 (≤ 2) | 2 |
| `crank_price` | `PumpMigrating → None` (migrated elsewhere; `graduated_elsewhere`) | 26338 | 26338 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | `None` (graduated elsewhere), the detection set, the S3 chain running | 16764 | 16764 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | Token-2022, 115 B curve (t22_115): `None → PumpCurve` re-detection (curve cache, price start, Tier D, chains and fp_* cleared) | 50826 | 50826 | — | 60000 | §6.1 `crank_price` | 4 | 2 (≤ 2) | 2 |
| `sweep_trader` | SPL Token, 115 B curve, no creator (legacy115): token leg — lamports on the trader, tokens on trader_base_ata | 22114 | 22114 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 4 | — | 2 |
| `sweep_trader` | Token-2022, 115 B curve (t22_115): token leg — lamports on the trader, tokens on trader_base_ata | 24638 | 24638 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 4 | — | 2 |
| `sweep_trader` | Token-2022, 115 B curve (t22_115): nothing to sweep | 18694 | 18694 | — | 60000 | no §6.1 row (§3.9); §6.1 `claim_sol` / `cover_deficit` / `sync_lender` class | 2 | — | 2 |
| `flag_venue_upgraded` | pump, the dumped 10,485,760-byte ProgramData | 6026 | 6026 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | pump-fees, the dumped 948,989-byte ProgramData | 6026 | 6026 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | PumpSwap, the dumped 10,485,760-byte ProgramData | 6026 | 6026 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | pump, the dumped ProgramData | 8438 | 8438 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | pump-fees, the dumped ProgramData | 8438 | 8438 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
