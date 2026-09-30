# M3 CU bench (Raydium)

Compute units of every Raydium-path instruction (ARCH §9 M3: T-RAY-CU, T-CU; M4: `propose_pool` and `execute_pool_switch`) of the **release** short.win (`anchor build --tools-version v1.57 --arch v3`) against the **dumped mainnet** Raydium AMM v4 and CPMM programs and pools (`tests/fixtures/`), measured with Mollusk 0.15.1 on account states built by the LiteSVM harness on the mainnet fork, with the same short.win, Raydium, SPL Token (p-token) and Token-2022 ELFs; LiteSVM 0.16.0 (mainnet feature set) runs the same instruction as a cross-check and gives its instruction-trace entries, its peak stack height and the Raydium program's own CU (its token CPIs included). AMM v4 is SPL Token only, so the Token-2022 rows are CPMM's. Written by `M3_CU_WRITE=1 cargo test --manifest-path tests/bench/m3_cu/Cargo.toml --test m3_cu` (README.md); the test fails when a Mollusk figure exceeds its budget, the two harnesses disagree, or a case does not execute.

| Instruction | Case | Mollusk CU | LiteSVM CU | Venue CPI CU | Budget | Budget source | Trace | ARCH §6.2 (Raydium) | Peak height |
|---|---|---|---|---|---|---|---|---|---|
| `create_market` | AMM v4, SPL (Fartcoin) | 67868 | 67868 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | CPMM, SPL (USELESS) | 65628 | 65628 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | CPMM, Token-2022 base | 76875 | 76875 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | CPMM, creator fee on the SOL side | 73138 | 73138 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | CPMM, WSOL as token 1 | 69707 | 69707 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `open_short` | AMM v4, SPL (Fartcoin): first open of a wallet (position + exposure created) | 101232 | 101232 | 15420 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, SPL (USELESS): first open of a wallet (position + exposure created) | 117000 | 117000 | 25086 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, Token-2022 base: first open of a wallet (position + exposure created) | 125198 | 125198 | 28019 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, creator fee on the SOL side: first open of a wallet (position + exposure created) | 116494 | 116494 | 25105 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, WSOL as token 1: first open of a wallet (position + exposure created) | 119268 | 119268 | 25088 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | AMM v4, SPL (Fartcoin): second position of a wallet | 99340 | 99340 | 15420 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 14 | 13 (≤ 15) | 3 |
| `open_short` | AMM v4, SPL (Fartcoin): first open, normalising (trader and trader_wsol donated to) | 107214 | 107214 | 15420 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 18 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, Token-2022 base: second position of a wallet | 119804 | 119804 | 28013 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 14 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, Token-2022 base: first open, normalising (trader and trader_wsol donated to) | 131180 | 131180 | 28019 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 18 | 13 (≤ 15) | 3 |
| `close_short` | AMM v4, SPL (Fartcoin) | 68792 | 68792 | 15900 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, SPL (USELESS) | 80225 | 80225 | 25178 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, Token-2022 base | 83772 | 83772 | 28074 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, creator fee on the SOL side | 80269 | 80269 | 25179 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, WSOL as token 1 | 80279 | 80279 | 25182 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | AMM v4, SPL (Fartcoin): normalising | 74773 | 74773 | 15900 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 12 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, Token-2022 base: normalising | 89753 | 89753 | 28074 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 12 | 9 (≤ 11) | 3 |
| `liquidate` | AMM v4, SPL (Fartcoin): after a real ≈ 2.2× pump, both legs, within the fixed bound | 91629 | 91629 | 16032 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `liquidate` | CPMM, SPL (USELESS): after a real ≈ 2.2× pump, both legs, within the fixed bound | 108313 | 108313 | 25231 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `liquidate` | CPMM, Token-2022 base: after a real ≈ 2.2× pump, both legs, within the fixed bound | 112295 | 112295 | 28141 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `expire` | AMM v4, SPL (Fartcoin): past the deadline, healthy | 87304 | 87304 | 15900 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `expire` | CPMM, SPL (USELESS): past the deadline, healthy | 102284 | 102284 | 25184 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `expire` | CPMM, Token-2022 base: past the deadline, healthy | 105812 | 105812 | 28080 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `sol_settle` | AMM v4, SPL (Fartcoin): healthy, the no-change call | 38988 | 38988 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | AMM v4, SPL (Fartcoin): S3 on a halted pool at the deadline, chain run s3_wait_s | 47830 | 47830 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `sol_settle` | CPMM, SPL (USELESS): healthy, the no-change call | 41372 | 41372 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | CPMM, SPL (USELESS): S3 on a halted pool at the deadline, chain run s3_wait_s | 53179 | 53179 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `sol_settle` | CPMM, Token-2022 base: healthy, the no-change call | 41721 | 41721 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | CPMM, Token-2022 base: S3 on a halted pool at the deadline, chain run s3_wait_s | 53525 | 53525 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `crank_price` | AMM v4, SPL (Fartcoin): roll + record (first crank of a second) | 28035 | 28035 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | AMM v4, SPL (Fartcoin): pool halted by its admin: roll, no record, both chains start | 26853 | 26853 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, SPL (USELESS): roll + record (first crank of a second) | 28093 | 28093 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, SPL (USELESS): pool halted by its admin: roll, no record, both chains start | 27508 | 27508 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, Token-2022 base: roll + record (first crank of a second) | 28104 | 28104 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, Token-2022 base: pool halted by its admin: roll, no record, both chains start | 27517 | 27517 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | AMM v4, SPL (Fartcoin): record only (same second again) | 23114 | 23114 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | AMM v4, the dumped 1,406,429-byte ProgramData | 6026 | 6026 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | CPMM, the dumped 793,869-byte ProgramData | 6026 | 6026 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | AMM v4, a second upgrade while flagged (refresh) | 6025 | 6025 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | a 10,485,760-byte ProgramData (pump's size, synthetic): U5 | 6026 | 6026 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | AMM v4, the dumped ProgramData | 8438 | 8438 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | CPMM, the dumped ProgramData | 8438 | 8438 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | AMM v4, SPL (Fartcoin): list-only market adopts it (Fartcoin's real curve: D1 detection) | 27055 | 27055 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, SPL (USELESS): list-only market adopts it | 27389 | 27389 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | AMM v4: listing-window replacement of the thin reversed Fartcoin pool by the main pool, ring rewritten | 45501 | 45501 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | AMM v4, SPL (Fartcoin): zero-position adoption of a planted CPMM pool 2× as deep, ring rewritten | 42257 | 42257 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, SPL (USELESS): zero-position adoption of a planted CPMM pool 2× as deep, ring rewritten | 42584 | 42584 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, Token-2022 base: zero-position adoption of a planted CPMM pool 2× as deep, ring rewritten | 42630 | 42630 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | AMM v4, SPL (Fartcoin): halted incumbent, a position open: fast-path window opened (CPMM candidate at 40 % of D) | 39469 | 39469 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | AMM v4, SPL (Fartcoin): fast-path window improved (60 % replaces 40 %) | 39080 | 39080 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, SPL (USELESS): halted incumbent, a position open: fast-path window opened (CPMM candidate at 40 % of D) | 40171 | 40171 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, SPL (USELESS): fast-path window improved (60 % replaces 40 %) | 39952 | 39952 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, Token-2022 base: halted incumbent, a position open: fast-path window opened (CPMM candidate at 40 % of D) | 39810 | 39810 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, Token-2022 base: fast-path window improved (60 % replaces 40 %) | 40059 | 40059 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | AMM v4, SPL (Fartcoin): fast adoption after the window, a position open, ring rewritten | 47814 | 47814 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | AMM v4, SPL (Fartcoin): window cancelled (the incumbent resumed) | 29957 | 29957 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | CPMM, SPL (USELESS): fast adoption after the window, a position open, ring rewritten | 48689 | 48689 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | CPMM, SPL (USELESS): window cancelled (the incumbent resumed) | 29828 | 29828 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | CPMM, Token-2022 base: fast adoption after the window, a position open, ring rewritten | 48771 | 48771 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | CPMM, Token-2022 base: window cancelled (the incumbent resumed) | 29814 | 29814 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | AMM v4, SPL (Fartcoin): the last post-change warm-up integration (seed over 8 entries, guard cleared; M4R) | 27210 | 27210 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, SPL (USELESS): the last post-change warm-up integration (seed over 8 entries, guard cleared; M4R) | 27205 | 27205 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, Token-2022 base: the last post-change warm-up integration (seed over 8 entries, guard cleared; M4R) | 27216 | 27216 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
