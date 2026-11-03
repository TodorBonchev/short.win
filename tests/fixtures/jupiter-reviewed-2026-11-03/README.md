# Jupiter deployment qualification — 2026-11-03

The selected fresh route matrix passed all 18 expected outcomes on both the exact 24-hour-fuzzed program ELF (`86147af0…`) and the root build (`b12fefdc…`). This is saved-route actual-SBF compatibility in LiteSVM with synthetic funding, financial policy/history, local signers and VM Clock. No mainnet transaction was signed or submitted. It is separate from the completed duration campaign, funded operation, and an external audit.

## Reviewed deployment

- Program: `JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4`
- ProgramData: `4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT`
- Deployment slot: `462802085`
- ProgramData length: `2892269` bytes
- Trimmed ELF SHA-256: `f09c1c20bc5734da6ba759acbb276c36a0b3e82306a60ff4b94341364aa0b895`

`deployment.json` binds finalized same-bank raw mainnet Program, ProgramData and Clock observations, exact retained bytes, canonical loader/address metadata and both padded/trimmed digest semantics. `post-qualification/deployment.json` independently observes the same deployment and exact program bytes after the test run.

## Selected fresh lifecycles

| Case | Sell / buy venues | Open / close bytes | Open / close CU |
| --- | --- | ---: | ---: |
| bonk-production | Whirlpool / Whirlpool | 1166 / 1086 | 143597 / 121661 |
| jup-production | Raydium CLMM / Whirlpool | 1173 / 1055 | 158835 / 114457 |
| ray-production-retry | Raydium CLMM / Raydium CLMM | 1139 / 1056 | 155547 / 134724 |
| pump-curve-production | Pump.fun / Pump.fun | 1175 / 1062 | 201139 / 151570 |
| pump-graduated-production | Pump.fun Amm / Pump.fun Amm | 1175 / 1061 | 178326 / 149687 |
| pump-metadata-active-Ai66-pumpamm | Pump.fun Amm / Pump.fun Amm | 1114 / 1000 | 199538 / 173982 |

Every selected positive case executed the real Jupiter CPI, signed fee-bound rollback, opening sale and full-quantity buyback. Actual captured lookup tables fit the 1232-byte packet bound; synthetic lookup fallback was not accepted. Token/native lamport conservation, principal/exposure/vault accounting, fees/collateral, setup/rent refunds, metadata and scratch cleanup passed. Pump routes also exercised four setup-budget refusals and zero-allowance closes.

Two explicit Scorch routes produced the exact expected downstream refusal with protected state unchanged. The predecessor reviewed deployment (`459479624`) was refused by real on-chain initialization before Jupiter CPI, leaving protected state unchanged. Each of these nine cases ran against both own-program ELFs.

## Preserved discovery outcomes

The initial 8GV Token-2022 curve capture contained only 15 lamports of real SOL and its complete token supply in the curve vault. Its synthetic funded-lender replay produced a downstream Pump overflow and remained a failed input. A second recently created mint lost real reserves/circulation before capture and was rejected by independent inventory checks. The first mature Ai66 quote used Meteora DLMM and hit Jupiter slippage protection on replay. These raw attempts remain in the dated capture corpus; none was relabelled as a positive pass. The final Ai66 capture explicitly uses Pump.fun AMM in both directions with unchanged 50bps slippage and 500bps reference spread bounds, over 10,888 SOL in observed quote reserves, and substantial external token circulation.

The supplemental historical-account/current-router overlay was diagnostic only. Its six positive historical inputs passed, while the two old Scorch inputs returned a different refusal after the Clock changed. That mixed-state attempt is retained in local ignored qualification state, is not the fresh matrix, and is not cited as passing qualification.

## Evidence and reproduction

- `qualification/fresh-suite/summary.json`: final 18-case report, hashes and exact process outcomes.
- `qualification/fresh-suite/retained-files.json`: bounded parent-side promotion index for per-case candidate manifests, process receipts, summaries and transaction logs.
- `qualification/own-artifacts.json`: exact own-ELF identities and identical executable text; only nine panic source-location bytes differ.
- `qualification/execution.json`: offline sandbox limits and successful native/config check receipts (71 Rust tests, 13 configuration tests, formatting and Clippy).
- `tests/runtime/current-routes-2026-11-03.json`: selected input matrix in the repository.

From the repository root, build the actual program and native replayer, then run:

```sh
cargo build -p shortwin-simple-runtime-tests --example qualify_current_route --locked
python3 scripts/qualify-reviewed-route-suite.py \
  --matrix tests/runtime/current-routes-2026-11-03.json \
  --reviewed-deployment tests/fixtures/jupiter-reviewed-2026-11-03/deployment.json \
  --own-program target/deploy/shortwin_simple.so \
  --runner target/debug/examples/qualify_current_route \
  --output target/reviewed-route-suite-new
```

The output directory must be new. Repeat `--own-program` to bind another exact ELF. The recorded primary release candidate is the separately preserved 24-hour-fuzzed ELF; a newly rebuilt binary does not inherit that duration claim. The CI workflow repeats this fresh saved-route suite against its own build. The configuration verifier checks the retained reviewed tuple and source/evidence hashes without network access. A later upstream change requires another review and replay before changing the pin.
