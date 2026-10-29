# Fresh production-ID route evidence — 2026-10-29

This record qualifies deterministic snapshot compatibility for program `BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh`, version 1.1.0. Fresh public Jupiter build requests used this program's actual loan/execution PDAs. The October 28 archive remains historical and unchanged; no old-program pass is substituted for this result.

The current SBF SHA256 is `d4868d29c14f4a04abe1cdead451f197b2102286fec10abe34161d387f66c510`; generated and built IDL SHA256 is `52bdccd8a9f29739885e95a4c4595e70067edf3572e9d60c23aa69741b6abc15`. The approved deterministic native replayer was rebuilt after its source was restored byte-for-byte to `a53af2fba`; its SHA256 is `4a7aa049b766cba3d6b9a6a30a92a9726adf47ee9dfb816724bb163a1d04cf72`.

## Actual execution result

[route-matrix.json](route-matrix.json) selects six positive cases and two explicit Scorch diagnostics. The [suite summary](runtime/deterministic-new-id-route-suite-20261029/summary.json) reports all eight expected outcomes verified, zero pending cases and zero live transactions. Summary SHA256: `dacc2083f4aaec49e15e7f9cb9337b98245cb4f352e3e88a8b4f6b622f3c9004`.

| Capture | Exact raw q | Sell/buy family | Net bid / bounded ask, lamports | Snapshot slot | Actual open/close wire bytes |
|---|---:|---|---:|---:|---:|
| bonk-production | 400000000000 | Whirlpool / Whirlpool | 123451750 / 129624337 | 462682449 | 1166 / 1052 |
| jup-production | 40000000 | Whirlpool / Whirlpool | 109399806 / 114869796 | 462682526 | 1166 / 1052 |
| ray-production-retry | 10000000 | Whirlpool / Raydium CLMM | 174814982 / 183555731 | 462683523 | 1076 / 1028 |
| pump-curve-production | 2500000000000 | Pump.fun / Pump.fun | 113215010 / 118875760 | 462682666 | 1175 / 1062 |
| pump-graduated-production | 3500000000000 | Pump.fun Amm / Pump.fun Amm | 124143337 / 130350503 | 462682771 | 1175 / 1061 |
| pump-metadata-current-8GV | 2663583160592 | Pump.fun / Pump.fun | 127315557 / 133681334 | 462690069 | 1176 / 1063 |

All six used the ordinary production request profile: account budget 16 with deterministic fallback to 32, Scorch excluded, no DEX allowlist. The high-volume pairs selected budget 16; the Pump pairs required 32. The strict program envelope remains 48 metas / 768 route bytes and the service limit is 40 metas. Every selected quote uses 50-bps slippage, ordinary RouteV2 headers and a live bid/ask spread ceiling of 500 bps. The replay also uses a 500-bps functional spread policy. It measures actual transaction serialization with captured real lookup tables; it does not infer packet fit from `maxAccounts`.

The six actual-SBF runs completed the opening sell and whole-q buyback on mutated local DEX state, retained at least q net tokens in the lender vault, checked principal/exposure/paid fees/collateral and base/native/WSOL conservation, closed scratch accounts, preserved real mint bytes, and verified full post-CPI rollback. Exact transaction logs and hashes are beside each replay summary.

Opening Pump setup costs were 2,199,360 / 1,844,400 / 1,844,400 lamports respectively, paid by the synthetic opening actor under its signed 5,000,000-lamport allowance; unused advances were refunded. All three same-loan buybacks passed with signed setup allowance zero and actual setup cost zero. These observations do not approve a positive production keeper subsidy or prove a funded operator; that financial decision remains separate.

The explicit `jup-scorch-negative` and `ray-scorch-negative` cases deliberately request Scorch without the production exclusion. Both actual downstream executions refuse with exact `Custom(5460)` / `0x1554`; the matrix verifies this refusal. They are negative compatibility evidence, not passing routes. The separate initial RAY attempt rejected a header/envelope; the later unrestricted production-profile retry selected a compatible route. The retained CLMM-only diagnostic is not presented as default-profile qualification.

## Pump state and physical inventory

The legacy SPL curve mint `8FxewF272LJ7Usov6PZk2gu1JKoR5Dq1v4sDqavJpump` has an actual Pump-owned incomplete curve. The graduated mint `Coyj3LtKn1BNSgWc9HsGK5SKoGfEoDaymig4wrN6pump` uses the actual current pAMM-owned pool `H739cDUEhbW2ckYhjetYSwxTVT5vd6GrTW5GdfaTQJgB`. These are state-based classifications.

The new metadata-only Token-2022 case is mint `8GVcbvQa1HtLvwH5ZScmrPyf2ByvamqQkoPkpbknpump`, with its actual 404-byte mint, no freeze authority and only MetadataPointer / TokenMetadata extensions (18 / 19). Its native-SOL curve `4B1DBR1peDySHiPx37nbopPgxLL8FwXyBgwU19Jbw4Xp` is incomplete. At account context 462690058 it held 9,636,840,976 real quote lamports; 260,876,752,594,622 tokens were outside its curve vault, exceeding the synthetic lender allocation 10q = 26,635,831,605,920. Discovery and capture retained the same curve state. [physical-bounds.json](pump-metadata-current-8GV/physical-bounds.json) and [physical-inventory-checks.json](physical-inventory-checks.json) retain the exact bounds. A sufficient physical bound is necessary for this synthetic test and does not establish real funded lender deposits.

The earlier `pump-metadata-production` mint `5PjPp8gDLjDsoXyvUtYzWKPYaRbfNaoxW29LTFvVpump` is excluded: its captured curve had only 56 real quote lamports and its curve vault held the entire 1e15 mint supply. Neither q nor 10q was available outside the curve. An apparently successful HTTP quote could not establish executable liquidity. Its full raw snapshot and [counterexample](pump-metadata-physical-bounds.json) are preserved. Other discovered metadata candidates and rejected requests remain diagnostic captures and receive no execution approval.

The historical plain-82 admission diagnostic in the capture tool flags metadata mints as `mint_eligible_plain82_rule=false` and labels them diagnostic. The actual current program, with its reviewed 18/19 parser, is authoritative; the selected 8GV mint passed actual `register_market` and subsequent Token-2022 CPIs. No raw mint or DEX account bytes were rewritten to make it pass.

## Current upstream bytes

[external/identity-conclusion.json](external/identity-conclusion.json) and [comparison-latest-pinned.json](external/comparison-latest-pinned.json) show all 22 refreshed executable/ProgramData byte hashes unchanged against the October 28 route evidence and latest pinned October 12 pump-fees baseline. Jupiter remains deployment slot 459479624, ProgramData length 2,892,269, full-data SHA256 `21e28c59e3b0a5536e48d31a1ca7346514a2bc61d70dc1388c920d7e3be35c1b` and trimmed ELF SHA256 `5e31d320f5d88ae93c7983ce82a1497067abd86ff540b35073abaeeb25ef397b`. SPL Token, Token-2022, Whirlpool, CLMM, Pump, PumpSwap and Scorch code identities are retained as raw RPC receipts.

The initial comparison used an ancient original pump-fees fixture and is preserved for provenance. The corrected comparison uses the current pinned `pump_fees_2026_10_12_program_data`: full bytes `3cf9ee9a380c0ced31171802d402551910ceb8c4819ede4ef44aaf63a12dbfac`, deployment slot 456132171 and authority `7gZufwwAo17y5kg8FMyJy2phgpvv9RSdzWtdXiWHjFr8`, all unchanged. No new upstream ELF or fixture promotion occurred.

Two fee-owned BuybackVault accounts changed mutable claimed counters / last-claimed bytes within offsets 40..71 only. Their authorities, claim-rate limits and reserved bytes are unchanged. The current official [Pump fees IDL](https://github.com/pump-fun/pump-public-docs/blob/main/idl/pump_fees.json), exact response bytes and receipt are archived; this counter drift is not an executable-code change.

## Reconstruction and integrity

`index.json` records original and stored hashes, sizes, request timestamps and slots. Raw RPC bodies are losslessly compressed. Eight selected cases plus the excluded 5Pj snapshot retain full RPC replies; additional request/discovery evidence retains its recorded scope. `external/index.json` separately pins the dependency refresh. `archive-files.sha256.json` covers every final archive file except itself.

From a clean clone, after building the matching SBF and approved deterministic replayer:

```sh
SHORTWIN_SIMPLE_SO="$PWD/simple/target/deploy/shortwin_simple.so" python3 simple/scripts/qualify-current-route-suite.py   --matrix docs/audit/simple-final-route-evidence-2026-10-29/route-matrix.json   --output simple/target/new-id-current-route-replay
```

The suite reconstructs decoded accounts from the recorded `getMultipleAccounts` requests and `.json.gz` replies, verifies all decoded hashes/owners/lamports/slots, and runs the existing normal two-argument deterministic replayer. A single capture can be reconstructed with `materialize-current-route-candidate.py CAPTURE NEW_OUTPUT --own-program-sha256 d4868d29c14f4a04abe1cdead451f197b2102286fec10abe34161d387f66c510`. Decoded binary duplicates need not be committed.

The exact capture tool is retained as `capture-tool.py`. Its legacy static `buy_search` prose mentions doubling; `production_observation=true` actually uses the source-pinned production branch starting at net bid and capping increases at the 500-bps bound. Selected asks and complete request history are authoritative; no cheapest-ask proof is claimed. Historical response fields `cpi_execution_qualified=false` / `history_qualified=false` correctly describe the HTTP capture stage; the later linked deterministic runtime reports supply the separate execution result.

## Limits of this evidence

The result covers the exact mint, quantity, route family, program bytes and captured states above, at roughly 0.1–0.18 SOL net bid. It is not a universal-mint guarantee or a 5-SOL capacity proof. Accounts can span RPC slots; no atomic mainnet landing or currently executable quote is promised. Jupiter routes, pool state and upstream deployments can change.

The replayer's local signers/funding, lender allocation, two-epoch history attestation and other functional policy inputs are synthetic. The test does not establish real production three-epoch history, service throughput, funded keeper operations, alert delivery or deployment readiness. No combined campaign, seeded-variant or 24-hour qualification was run or continued in this work; the provider-stopped preparation remains unqualified and separate. This is independent AI review and deterministic compatibility evidence, not an external-firm audit.
