# M3 CU bench (Raydium)

Compute units of every Raydium-path instruction (ARCH §9 M3: T-RAY-CU, T-CU; M4: `propose_pool` and `execute_pool_switch`) of the **release** short.win (`anchor build --tools-version v1.57 --arch v3`) against the **dumped mainnet** Raydium AMM v4 and CPMM programs and pools (`tests/fixtures/`), measured with Mollusk 0.15.1 on account states built by the LiteSVM harness on the mainnet fork, with the same short.win, Raydium, SPL Token (p-token) and Token-2022 ELFs; LiteSVM 0.16.0 (mainnet feature set) runs the same instruction as a cross-check and gives its instruction-trace entries, its peak stack height and the Raydium program's own CU (its token CPIs included). AMM v4 is SPL Token only, so the Token-2022 rows are CPMM's. Written by `M3_CU_WRITE=1 cargo test --manifest-path tests/bench/m3_cu/Cargo.toml --test m3_cu` (README.md); the test fails when a Mollusk figure exceeds its budget, the two harnesses disagree, or a case does not execute.

| Instruction | Case | Mollusk CU | LiteSVM CU | Venue CPI CU | Budget | Budget source | Trace | ARCH §6.2 (Raydium) | Peak height |
|---|---|---|---|---|---|---|---|---|---|
| `create_market` | AMM v4, SPL (Fartcoin) | 67856 | 67856 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | CPMM, SPL (USELESS) | 65622 | 65622 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | CPMM, Token-2022 base | 76869 | 76869 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | CPMM, creator fee on the SOL side | 73132 | 73132 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | CPMM, WSOL as token 1 | 69700 | 69700 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `open_short` | AMM v4, SPL (Fartcoin): first open of a wallet (position + exposure created) | 101165 | 101165 | 15420 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, SPL (USELESS): first open of a wallet (position + exposure created) | 116942 | 116942 | 25086 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, Token-2022 base: first open of a wallet (position + exposure created) | 125140 | 125140 | 28019 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, creator fee on the SOL side: first open of a wallet (position + exposure created) | 116436 | 116436 | 25105 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, WSOL as token 1: first open of a wallet (position + exposure created) | 119208 | 119208 | 25088 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | AMM v4, SPL (Fartcoin): second position of a wallet | 99273 | 99273 | 15420 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 14 | 13 (≤ 15) | 3 |
| `open_short` | AMM v4, SPL (Fartcoin): first open, normalising (trader and trader_wsol donated to) | 107147 | 107147 | 15420 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 18 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, Token-2022 base: second position of a wallet | 119746 | 119746 | 28013 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 14 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, Token-2022 base: first open, normalising (trader and trader_wsol donated to) | 131122 | 131122 | 28019 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 18 | 13 (≤ 15) | 3 |
| `close_short` | AMM v4, SPL (Fartcoin) | 68759 | 68759 | 15900 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, SPL (USELESS) | 80194 | 80194 | 25178 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, Token-2022 base | 83741 | 83741 | 28074 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, creator fee on the SOL side | 80238 | 80238 | 25179 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, WSOL as token 1 | 80247 | 80247 | 25182 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | AMM v4, SPL (Fartcoin): normalising | 74740 | 74740 | 15900 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 12 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, Token-2022 base: normalising | 89722 | 89722 | 28074 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 12 | 9 (≤ 11) | 3 |
| `liquidate` | AMM v4, SPL (Fartcoin): after a real ≈ 2.2× pump, both legs, within the fixed bound | 91511 | 91511 | 16032 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `liquidate` | CPMM, SPL (USELESS): after a real ≈ 2.2× pump, both legs, within the fixed bound | 108197 | 108197 | 25231 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `liquidate` | CPMM, Token-2022 base: after a real ≈ 2.2× pump, both legs, within the fixed bound | 112177 | 112177 | 28141 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `expire` | AMM v4, SPL (Fartcoin): past the deadline, healthy | 87186 | 87186 | 15900 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `expire` | CPMM, SPL (USELESS): past the deadline, healthy | 102168 | 102168 | 25184 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `expire` | CPMM, Token-2022 base: past the deadline, healthy | 105694 | 105694 | 28080 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `sol_settle` | AMM v4, SPL (Fartcoin): healthy, the no-change call | 38836 | 38836 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | AMM v4, SPL (Fartcoin): S3 on a halted pool at the deadline, chain run s3_wait_s | 47682 | 47682 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `sol_settle` | CPMM, SPL (USELESS): healthy, the no-change call | 41225 | 41225 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | CPMM, SPL (USELESS): S3 on a halted pool at the deadline, chain run s3_wait_s | 53033 | 53033 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `sol_settle` | CPMM, Token-2022 base: healthy, the no-change call | 41572 | 41572 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | CPMM, Token-2022 base: S3 on a halted pool at the deadline, chain run s3_wait_s | 53377 | 53377 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `crank_price` | AMM v4, SPL (Fartcoin): roll + record (first crank of a second) | 27884 | 27884 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | AMM v4, SPL (Fartcoin): pool halted by its admin: roll, no record, both chains start | 26702 | 26702 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, SPL (USELESS): roll + record (first crank of a second) | 27947 | 27947 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, SPL (USELESS): pool halted by its admin: roll, no record, both chains start | 27359 | 27359 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, Token-2022 base: roll + record (first crank of a second) | 27956 | 27956 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, Token-2022 base: pool halted by its admin: roll, no record, both chains start | 27366 | 27366 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | AMM v4, SPL (Fartcoin): record only (same second again) | 22963 | 22963 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | AMM v4, the dumped 1,406,429-byte ProgramData | 6026 | 6026 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | CPMM, the dumped 793,869-byte ProgramData | 6026 | 6026 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | AMM v4, a second upgrade while flagged (refresh) | 6025 | 6025 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | a 10,485,760-byte ProgramData (pump's size, synthetic): U5 | 6026 | 6026 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | AMM v4, the dumped ProgramData | 8438 | 8438 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | CPMM, the dumped ProgramData | 8438 | 8438 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | AMM v4, SPL (Fartcoin): list-only market adopts it (Fartcoin's real curve: D1 detection) | 27049 | 27049 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, SPL (USELESS): list-only market adopts it | 27393 | 27393 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | AMM v4: listing-window replacement of the thin reversed Fartcoin pool by the main pool, ring rewritten | 45518 | 45518 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | AMM v4, SPL (Fartcoin): zero-position adoption of a planted CPMM pool 2× as deep, ring rewritten | 42280 | 42280 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, SPL (USELESS): zero-position adoption of a planted CPMM pool 2× as deep, ring rewritten | 42612 | 42612 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, Token-2022 base: zero-position adoption of a planted CPMM pool 2× as deep, ring rewritten | 42658 | 42658 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | AMM v4, SPL (Fartcoin): halted incumbent, a position open: fast-path window opened (CPMM candidate at 40 % of D) | 39501 | 39501 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | AMM v4, SPL (Fartcoin): fast-path window improved (60 % replaces 40 %) | 39112 | 39112 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, SPL (USELESS): halted incumbent, a position open: fast-path window opened (CPMM candidate at 40 % of D) | 40205 | 40205 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, SPL (USELESS): fast-path window improved (60 % replaces 40 %) | 39986 | 39986 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, Token-2022 base: halted incumbent, a position open: fast-path window opened (CPMM candidate at 40 % of D) | 39844 | 39844 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, Token-2022 base: fast-path window improved (60 % replaces 40 %) | 40093 | 40093 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | AMM v4, SPL (Fartcoin): fast adoption after the window, a position open, ring rewritten | 47841 | 47841 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | AMM v4, SPL (Fartcoin): window cancelled (the incumbent resumed) | 29995 | 29995 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | CPMM, SPL (USELESS): fast adoption after the window, a position open, ring rewritten | 48718 | 48718 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | CPMM, SPL (USELESS): window cancelled (the incumbent resumed) | 29871 | 29871 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | CPMM, Token-2022 base: fast adoption after the window, a position open, ring rewritten | 48800 | 48800 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | CPMM, Token-2022 base: window cancelled (the incumbent resumed) | 29857 | 29857 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | AMM v4, SPL (Fartcoin): the last post-change warm-up integration (seed over 8 entries, guard cleared; M4R) | 27064 | 27064 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, SPL (USELESS): the last post-change warm-up integration (seed over 8 entries, guard cleared; M4R) | 27059 | 27059 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, Token-2022 base: the last post-change warm-up integration (seed over 8 entries, guard cleared; M4R) | 27068 | 27068 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
