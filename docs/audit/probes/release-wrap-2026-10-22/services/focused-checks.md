# Focused integration checks

Source: `/home/todor_dev/Projects/short.win-release-devnet`, branch `release/1.0.3-devnet-prep-2026-10-22`, initial clean HEAD `b31c02a091008da18c39d7836c8e0e9d6d018973`. Checks below ran 2026-10-22 local; commands set `NO_DNA=1`. Logs are complete siblings of this record. No target source was edited by this agent at this checkpoint.

| Check | Actual result | Log |
|---|---|---|
| `python3 scripts/audit_scope.py check` | Exit 0; watched tracked files classified and scope list matches HEAD | `audit-scope.log` |
| `python3 scripts/dump-fixtures.py check` | Exit 0; 344 fixture entries match files/pins/checksums | `fixture-static.log` |
| `pnpm install --offline --frozen-lockfile --ignore-scripts` | Exit 1; pnpm policy metadata cache lacks `@codama/renderers-js`; no version-resolution failure alleged | `pnpm-offline-install.log` |
| `pnpm install --frozen-lockfile --ignore-scripts` | Exit 0; 316 policy entries passed, 284 cached packages reused, no package downloads; lifecycle scripts intentionally disabled to avoid client/codegen output mutation | `pnpm-locked-install.log` |
| `pnpm --filter @shortwin/client check` | Exit 0; typecheck and all 30 tests passed, including 304-byte guard/all four full digests | `client-check.log` |
| `pnpm --filter web exec vitest run src/config/parseConfig.test.ts src/config/programs.test.ts src/features/short/venueGuard.test.ts src/api/client.test.ts src/api/ws.test.ts src/api/live.test.ts` | Exit 0; exactly six test files and 22 tests passed; browser clients only, no indexer server tests | `web-focused-baseline.log` |
| Direct Node import/call to existing `parseConfig` with public devnet URLs | Exit 0 and reproduced defect: `{ "cluster": "devnet", "explorerTxUrl": "https://explorer.solana.com/tx/{sig}" }` | `devnet-explorer-baseline.log` |
| `CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=<external>/target-services timeout 180s cargo test --manifest-path services/Cargo.toml --locked --offline -p shortwin-keeper --lib events::tests::logged_events_are_attributed_to_the_emitting_program -- --exact` | Exit 0; 1 test passed, 95 filtered; cold compile 1m27s, no shared target/artifact output | `keeper-event-auth.log` |
| Same six-file direct Vitest invocation after Explorer patch | Exit 0; exactly six test files and 24 tests passed | `web-focused-patched.log` |
| External Codama rerender from fresh locked/offline program IDL, original visitor/options, absolute input/output paths only | Exit 0; generated source directory diff against committed source also exit 0 | `client-codegen-fresh.log`, `client-codegen-diff.log`, `codama-check.json` |
| Fresh `pnpm --filter @shortwin/math-wasm build:force`, `CARGO_TARGET_DIR=target/wasm-readiness-2026-10-22`, offline/locked, two jobs | Exit 0; 25.17s compile, exact wasm-bindgen 0.2.129 | `wasm-fresh-build.log` |
| Node-target bindgen of same freshly built raw WASM, existing `crates/shortwin-math-wasm/js/smoke.cjs` | Exit 0; `facade smoke test: ok` | `wasm-node-smoke.log` |
| `pnpm --filter web build` with explicit localnet loopback configuration, test venue disabled, MWA enabled | Exit 0; includes current WASM cache check, TypeScript noEmit check and Vite production bundling | `web-production-build.log` |
| Biome check of the two patched config files | Exit 0; no fixes needed | `web-config-biome.log` |
| `git diff --check` after patch and build | Exit 0 | `final-diff-check.log` |

Pinned available tools observed: Node v24.21.0, pnpm 12.7.0, wasm-bindgen 0.2.129. Source remained clean after installation/checks. No `.env` or key file was inspected.

Audit-scope interpretation: `WATCHED_ROOTS = ["programs", "crates", "services"]` plus the two runtime Cargo workspace members. `docs/design/article-economics-2026-10-22/Cargo.toml` declares a standalone `[workspace]`; those article/research artifacts and `docs/audit/2026-10-22-progress` evidence are outside watched roots. Green scope does not validate their native-proof claims or turn them into reviewed runtime components. Stopped evidence/fixtures were not inspected or reconstructed.

The baseline assertions did not cover the Explorer defect; the direct configuration probe confirmed it. Root then authorized and this agent applied the minimal source patch: `apps/web/src/config/parseConfig.ts` uses the devnet Explorer suffix for the default when `cluster === 'devnet'`; existing `parseConfig.test.ts` checks devnet/mainnet/localnet defaults and preserves an explicitly configured devnet template. Patch is retained as `explorer-default.patch` and is uncommitted by this agent. No API/server, transaction, venue, wallet, or compiled-program source change was made by this agent.

Program agent supplied fresh IDL at `target/program-readiness-2026-10-22/idl/shortwin.json`; direct diff against committed IDL passed and both hash to `5b94b1e98f5b32cf6d3c501d1998965bbbab80d6e2853fa6eeb8f3ba5470a003`. External generation writes no repository client source. Fresh ignored frontend WASM SHA-256 is `ebe959d8ccaba2718a8c0c60b8514145f588eb392e5454fbcf3bc1c13d15e027` (not claimed portable across build paths). Web build used localnet compile settings and is not a configured devnet deployment or live service test. Dist and sourcemaps are generated ignored outputs.

The separate program reviewer reported that the article-only rustdoc line shift changes a release ELF error-origin line-number byte. This report makes no old/new SBF byte-equivalence claim; program evidence and any root-approved source correction belong to that agent's artifact record.

The inherited CI/fuzz evidence retains its original source/runtime attribution. These new checks concern the integrated revision's install/client/browser configuration and static inventories; no full final-commit CI, mainnet launch, live devnet operation or completed security audit is claimed.
