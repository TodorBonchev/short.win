# M3 CU bench (Raydium)

Compute units of every Raydium-path instruction (ARCH §9 M3: T-RAY-CU, T-CU; M4: `propose_pool` and `execute_pool_switch`) of the **release** short.win (`anchor build --tools-version v1.57 --arch v3`) against the **dumped mainnet** Raydium AMM v4 and CPMM programs and pools (`tests/fixtures/`), measured with Mollusk 0.15.1 on account states built by the LiteSVM harness on the mainnet fork, with the same short.win, Raydium, SPL Token (p-token) and Token-2022 ELFs; LiteSVM 0.16.0 (mainnet feature set) runs the same instruction as a cross-check and gives its instruction-trace entries, its peak stack height and the Raydium program's own CU (its token CPIs included). AMM v4 is SPL Token only, so the Token-2022 rows are CPMM's. Written by `M3_CU_WRITE=1 cargo test --manifest-path tests/bench/m3_cu/Cargo.toml --test m3_cu` (README.md); the test fails when a Mollusk figure exceeds its budget, the two harnesses disagree, or a case does not execute.

| Instruction | Case | Mollusk CU | LiteSVM CU | Venue CPI CU | Budget | Budget source | Trace | ARCH §6.2 (Raydium) | Peak height |
|---|---|---|---|---|---|---|---|---|---|
| `create_market` | AMM v4, SPL (Fartcoin) | 63253 | 63253 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | CPMM, SPL (USELESS) | 64017 | 64017 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | CPMM, Token-2022 base | 75264 | 75264 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | CPMM, creator fee on the SOL side | 71527 | 71527 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | CPMM, WSOL as token 1 | 68095 | 68095 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `open_short` | AMM v4, SPL (Fartcoin): first open of a wallet (position + exposure created) | 100615 | 100615 | 15420 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, SPL (USELESS): first open of a wallet (position + exposure created) | 116393 | 116393 | 25086 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, Token-2022 base: first open of a wallet (position + exposure created) | 124591 | 124591 | 28019 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, creator fee on the SOL side: first open of a wallet (position + exposure created) | 115887 | 115887 | 25105 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, WSOL as token 1: first open of a wallet (position + exposure created) | 118659 | 118659 | 25088 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | AMM v4, SPL (Fartcoin): second position of a wallet | 98723 | 98723 | 15420 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 14 | 13 (≤ 15) | 3 |
| `open_short` | AMM v4, SPL (Fartcoin): first open, normalising (trader and trader_wsol donated to) | 106596 | 106596 | 15420 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 18 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, Token-2022 base: second position of a wallet | 119197 | 119197 | 28013 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 14 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, Token-2022 base: first open, normalising (trader and trader_wsol donated to) | 130572 | 130572 | 28019 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 18 | 13 (≤ 15) | 3 |
| `close_short` | AMM v4, SPL (Fartcoin) | 68286 | 68286 | 15900 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, SPL (USELESS) | 79723 | 79723 | 25178 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, Token-2022 base | 83270 | 83270 | 28074 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, creator fee on the SOL side | 79767 | 79767 | 25179 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, WSOL as token 1 | 79776 | 79776 | 25182 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | AMM v4, SPL (Fartcoin): normalising | 74266 | 74266 | 15900 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 12 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, Token-2022 base: normalising | 89250 | 89250 | 28074 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 12 | 9 (≤ 11) | 3 |
| `liquidate` | AMM v4, SPL (Fartcoin): after a real ≈ 2.2× pump, both legs, within the fixed bound | 90960 | 90960 | 16032 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `liquidate` | CPMM, SPL (USELESS): after a real ≈ 2.2× pump, both legs, within the fixed bound | 107648 | 107648 | 25231 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `liquidate` | CPMM, Token-2022 base: after a real ≈ 2.2× pump, both legs, within the fixed bound | 111628 | 111628 | 28141 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `expire` | AMM v4, SPL (Fartcoin): past the deadline, healthy | 86616 | 86616 | 15900 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `expire` | CPMM, SPL (USELESS): past the deadline, healthy | 101600 | 101600 | 25184 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `expire` | CPMM, Token-2022 base: past the deadline, healthy | 105126 | 105126 | 28080 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `sol_settle` | AMM v4, SPL (Fartcoin): healthy, the no-change call | 39390 | 39390 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | AMM v4, SPL (Fartcoin): S3 on a halted pool at the deadline, chain run s3_wait_s | 47431 | 47431 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `sol_settle` | CPMM, SPL (USELESS): healthy, the no-change call | 42698 | 42698 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | CPMM, SPL (USELESS): S3 on a halted pool at the deadline, chain run s3_wait_s | 52786 | 52786 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `sol_settle` | CPMM, Token-2022 base: healthy, the no-change call | 43042 | 43042 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | CPMM, Token-2022 base: S3 on a halted pool at the deadline, chain run s3_wait_s | 53130 | 53130 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `crank_price` | AMM v4, SPL (Fartcoin): roll + record (first crank of a second) | 27823 | 27823 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | AMM v4, SPL (Fartcoin): pool halted by its admin: roll, no record, both chains start | 26640 | 26640 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, SPL (USELESS): roll + record (first crank of a second) | 27890 | 27890 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, SPL (USELESS): pool halted by its admin: roll, no record, both chains start | 27301 | 27301 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, Token-2022 base: roll + record (first crank of a second) | 27899 | 27899 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, Token-2022 base: pool halted by its admin: roll, no record, both chains start | 27308 | 27308 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | AMM v4, SPL (Fartcoin): record only (same second again) | 22902 | 22902 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | AMM v4, the dumped 1,406,429-byte ProgramData | 6026 | 6026 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | CPMM, the dumped 793,869-byte ProgramData | 6026 | 6026 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | AMM v4, a second upgrade while flagged (refresh) | 6025 | 6025 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | a 10,485,760-byte ProgramData (pump's size, synthetic): U5 | 6026 | 6026 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | AMM v4, the dumped ProgramData | 8438 | 8438 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | CPMM, the dumped ProgramData | 8438 | 8438 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | AMM v4, SPL (Fartcoin): list-only market adopts it (Fartcoin's real curve: D1 detection) | 27053 | 27053 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, SPL (USELESS): list-only market adopts it | 27418 | 27418 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | AMM v4: listing-window replacement of the thin reversed Fartcoin pool by the main pool, ring rewritten | 45584 | 45584 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | AMM v4, SPL (Fartcoin): zero-position adoption of a planted CPMM pool 2× as deep, ring rewritten | 42344 | 42344 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, SPL (USELESS): zero-position adoption of a planted CPMM pool 2× as deep, ring rewritten | 42678 | 42678 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, Token-2022 base: zero-position adoption of a planted CPMM pool 2× as deep, ring rewritten | 42724 | 42724 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | AMM v4, SPL (Fartcoin): halted incumbent, a position open: fast-path window opened (CPMM candidate at 40 % of D) | 39564 | 39564 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | AMM v4, SPL (Fartcoin): fast-path window improved (60 % replaces 40 %) | 39175 | 39175 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, SPL (USELESS): halted incumbent, a position open: fast-path window opened (CPMM candidate at 40 % of D) | 40270 | 40270 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, SPL (USELESS): fast-path window improved (60 % replaces 40 %) | 40051 | 40051 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, Token-2022 base: halted incumbent, a position open: fast-path window opened (CPMM candidate at 40 % of D) | 39909 | 39909 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `propose_pool` | CPMM, Token-2022 base: fast-path window improved (60 % replaces 40 %) | 40158 | 40158 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | AMM v4, SPL (Fartcoin): fast adoption after the window, a position open, ring rewritten | 47854 | 47854 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | AMM v4, SPL (Fartcoin): window cancelled (the incumbent resumed) | 29972 | 29972 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | CPMM, SPL (USELESS): fast adoption after the window, a position open, ring rewritten | 48733 | 48733 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | CPMM, SPL (USELESS): window cancelled (the incumbent resumed) | 29850 | 29850 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | CPMM, Token-2022 base: fast adoption after the window, a position open, ring rewritten | 48815 | 48815 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `execute_pool_switch` | CPMM, Token-2022 base: window cancelled (the incumbent resumed) | 29836 | 29836 | — | 100000 | §6.1 `propose_pool` / `execute_pool_switch` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | AMM v4, SPL (Fartcoin): the last post-change warm-up integration (seed over 8 entries, guard cleared; M4R) | 27007 | 27007 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, SPL (USELESS): the last post-change warm-up integration (seed over 8 entries, guard cleared; M4R) | 27002 | 27002 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, Token-2022 base: the last post-change warm-up integration (seed over 8 entries, guard cleared; M4R) | 27011 | 27011 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
