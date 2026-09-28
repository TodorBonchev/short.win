# M3 CU bench (Raydium)

Compute units of every Raydium-path instruction (ARCH §9 M3: T-RAY-CU, T-CU) of the **release** short.win (`anchor build --tools-version v1.57 --arch v3`) against the **dumped mainnet** Raydium AMM v4 and CPMM programs and pools (`tests/fixtures/`), measured with Mollusk 0.15.1 on account states built by the LiteSVM harness on the mainnet fork, with the same short.win, Raydium, SPL Token (p-token) and Token-2022 ELFs; LiteSVM 0.16.0 (mainnet feature set) runs the same instruction as a cross-check and gives its instruction-trace entries, its peak stack height and the Raydium program's own CU (its token CPIs included). AMM v4 is SPL Token only, so the Token-2022 rows are CPMM's. Written by `M3_CU_WRITE=1 cargo test --manifest-path tests/bench/m3_cu/Cargo.toml --test m3_cu` (README.md); the test fails when a Mollusk figure exceeds its budget, the two harnesses disagree, or a case does not execute.

| Instruction | Case | Mollusk CU | LiteSVM CU | Venue CPI CU | Budget | Budget source | Trace | ARCH §6.2 (Raydium) | Peak height |
|---|---|---|---|---|---|---|---|---|---|
| `create_market` | AMM v4, SPL (Fartcoin) | 61201 | 61201 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | CPMM, SPL (USELESS) | 61969 | 61969 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | CPMM, Token-2022 base | 73216 | 73216 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | CPMM, creator fee on the SOL side | 69478 | 69478 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `create_market` | CPMM, WSOL as token 1 | 66047 | 66047 | — | 250000 | §6.1 `create_market` | 7 | — | 2 |
| `open_short` | AMM v4, SPL (Fartcoin): first open of a wallet (position + exposure created) | 100599 | 100599 | 15420 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, SPL (USELESS): first open of a wallet (position + exposure created) | 116377 | 116377 | 25086 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, Token-2022 base: first open of a wallet (position + exposure created) | 124575 | 124575 | 28019 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, creator fee on the SOL side: first open of a wallet (position + exposure created) | 115871 | 115871 | 25105 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, WSOL as token 1: first open of a wallet (position + exposure created) | 118643 | 118643 | 25088 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 15 | 13 (≤ 15) | 3 |
| `open_short` | AMM v4, SPL (Fartcoin): second position of a wallet | 98708 | 98708 | 15420 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 14 | 13 (≤ 15) | 3 |
| `open_short` | AMM v4, SPL (Fartcoin): first open, normalising (trader and trader_wsol donated to) | 106584 | 106584 | 15420 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 18 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, Token-2022 base: second position of a wallet | 119182 | 119182 | 28013 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 14 | 13 (≤ 15) | 3 |
| `open_short` | CPMM, Token-2022 base: first open, normalising (trader and trader_wsol donated to) | 130560 | 130560 | 28019 | 150000 | §6.1 `open_short` RaydiumV4 / CPMM | 18 | 13 (≤ 15) | 3 |
| `close_short` | AMM v4, SPL (Fartcoin) | 68270 | 68270 | 15900 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, SPL (USELESS) | 79707 | 79707 | 25178 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, Token-2022 base | 83254 | 83254 | 28074 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, creator fee on the SOL side | 79751 | 79751 | 25179 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, WSOL as token 1 | 79760 | 79760 | 25182 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `close_short` | AMM v4, SPL (Fartcoin): normalising | 74254 | 74254 | 15900 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 12 | 9 (≤ 11) | 3 |
| `close_short` | CPMM, Token-2022 base: normalising | 89238 | 89238 | 28074 | 200000 | no Raydium `close_short` row; §6.1 `liquidate` / `expire` Raydium | 12 | 9 (≤ 11) | 3 |
| `liquidate` | AMM v4, SPL (Fartcoin): after a real ≈ 2.2× pump, both legs, within the fixed bound | 90505 | 90505 | 16032 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `liquidate` | CPMM, SPL (USELESS): after a real ≈ 2.2× pump, both legs, within the fixed bound | 107193 | 107193 | 25231 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `liquidate` | CPMM, Token-2022 base: after a real ≈ 2.2× pump, both legs, within the fixed bound | 111173 | 111173 | 28141 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `expire` | AMM v4, SPL (Fartcoin): past the deadline, healthy | 86112 | 86112 | 15900 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `expire` | CPMM, SPL (USELESS): past the deadline, healthy | 101096 | 101096 | 25184 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `expire` | CPMM, Token-2022 base: past the deadline, healthy | 104622 | 104622 | 28080 | 200000 | §6.1 `liquidate` / `expire` Raydium | 9 | 9 (≤ 11) | 3 |
| `sol_settle` | AMM v4, SPL (Fartcoin): healthy, the no-change call | 38389 | 38389 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | AMM v4, SPL (Fartcoin): S3 on a halted pool at the deadline, chain run s3_wait_s | 47377 | 47377 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `sol_settle` | CPMM, SPL (USELESS): healthy, the no-change call | 41697 | 41697 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | CPMM, SPL (USELESS): S3 on a halted pool at the deadline, chain run s3_wait_s | 52732 | 52732 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `sol_settle` | CPMM, Token-2022 base: healthy, the no-change call | 42041 | 42041 | — | 150000 | §6.1 `sol_settle` | 1 | 2 (≤ 5) | 1 |
| `sol_settle` | CPMM, Token-2022 base: S3 on a halted pool at the deadline, chain run s3_wait_s | 53076 | 53076 | — | 150000 | §6.1 `sol_settle` | 2 | 2 (≤ 5) | 2 |
| `crank_price` | AMM v4, SPL (Fartcoin): roll + record (first crank of a second) | 27746 | 27746 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | AMM v4, SPL (Fartcoin): pool halted by its admin: roll, no record, both chains start | 26567 | 26567 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, SPL (USELESS): roll + record (first crank of a second) | 27810 | 27810 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, SPL (USELESS): pool halted by its admin: roll, no record, both chains start | 27225 | 27225 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, Token-2022 base: roll + record (first crank of a second) | 27819 | 27819 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `crank_price` | CPMM, Token-2022 base: pool halted by its admin: roll, no record, both chains start | 27232 | 27232 | — | 60000 | §6.1 `crank_price` | 3 | 2 (≤ 2) | 2 |
| `crank_price` | AMM v4, SPL (Fartcoin): record only (same second again) | 22894 | 22894 | — | 60000 | §6.1 `crank_price` | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | AMM v4, the dumped 1,406,429-byte ProgramData | 6032 | 6032 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | CPMM, the dumped 793,869-byte ProgramData | 6032 | 6032 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | AMM v4, a second upgrade while flagged (refresh) | 6031 | 6031 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `flag_venue_upgraded` | a 10,485,760-byte ProgramData (pump's size, synthetic): U5 | 6032 | 6032 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | AMM v4, the dumped ProgramData | 8440 | 8440 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
| `revalidate_venue` | CPMM, the dumped ProgramData | 8440 | 8440 | — | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` (U5) | 2 | 2 (≤ 2) | 2 |
