# Local devnet service staging — 2026-10-22

The program-only 1.0.3 deployment is complete. This record covers **local service preparation**, not protocol initialization or a market smoke. The service build read Git `8a03f2a4d40d5364a7df0b0d0be5baf64325ea6a` on `release/1.0.3-devnet-prep-2026-10-22` and used the checked-out `services/Cargo.lock` (SHA-256 `3f0d57d6b12f62ebe5de5a10116fe528cd72fcb25663eee2f5b430706f2293c4`). There were no program-source changes in this staging step.

## Completed local checks

| Item | Observed result |
|---|---|
| Docker access | `newgrp docker` followed by `docker info --format '{{.ServerVersion}}'` returned `29.9.1`. The current shell did not inherit the recently added `docker` group, and `sg` is absent; no Docker container or image was needed here. |
| Source-matched service build | `NO_DNA=1 CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=<external target-services> cargo build --manifest-path services/Cargo.toml --locked --offline -p shortwin-indexer -p shortwin-keeper --bins --release` exited 0 in 4m 12s with Rust 1.98.1. The `shortwin-indexer` binary SHA-256 is `9fb16ff3963d2f78eff0415928c2dc9bac9130c806742ea23b24470b3d3b536a`; the `shortwin-keeper` binary SHA-256 is `4c7a6a6a28910a68f376f35c40bcbc39501434a82d6aa833723e82a8ec751429`. Both are under `/home/todor_dev/security-audit-skill/short.win/release-wrap-2026-10-22/release_services_readiness/target-services/release/`. The keeper was **not started**. |
| Local database | Cached PostgreSQL **17.11** initialized at `/home/todor_dev/.local/state/shortwin-devnet-1.0.3/data`. It was listening only on `127.0.0.1:5432` at the final check. Data, socket and config directories are mode `0700`; database password and service TOML files are mode `0600`, outside Git. The database is `shortwin_devnet`. Separate `shortwin_ingest` and `shortwin_api` logins authenticated over TCP. The API login reported `default_transaction_read_only=on` and `statement_timeout=2s`. |
| Migrations and grants | Source-matched `shortwin-indexer --config <private indexer-ingest.toml> migrate` exited 0. `_sqlx_migrations` contains **9 successful migrations**, versions 1 through 101; the public schema has **42 base tables**. The API role could select from `ingest_state` and was denied `CREATE TABLE` by its read-only transaction setting. |
| Reader smoke | The source-matched binary was started with the private API config binding `127.0.0.1:8080`; `GET /v1/health`, `/v1/status`, and `/v1/markets?limit=1` each returned HTTP 200. Status reported `confirmed_slot=0`, `finalized_slot=0`, `latest_block_time=null`, `streams=[]`, and no recorded warnings; markets returned an empty list. These are **empty-database responses**, not evidence of live chain ingest or market readiness. The API process was stopped after the smoke. `trusted_proxies=[]` is intentional for a direct loopback client with no load balancer. |
| Devnet browser build | `NO_DNA=1 pnpm --filter web build` passed with `VITE_CLUSTER=devnet`, public devnet RPC/WS, reader URLs `http://127.0.0.1:8080` and `ws://127.0.0.1:8080/v1/ws`, `VITE_ENABLE_TEST_VENUE=false`, and the devnet Explorer template. The resulting JavaScript contains those selected URLs. `pnpm --filter web preview --host 127.0.0.1 --port 4173 --strictPort` returned HTTP 200 from loopback and was stopped. `dist/index.html` hashes to `7f92980272c4d05aa0121d2ed6ccbb754434ed47ccf7e03c77ab7fd960ef87f8`; its WASM asset hashes to `ebe959d8ccaba2718a8c0c60b8514145f588eb392e5454fbcf3bc1c13d15e027`. This is a local build, not a published frontend or a wallet trade test. |
| 1.0.3 layout checks | `clients/js/test/layout.test.ts` passed 18/18, including full 304-byte `VenueGuard` round-trip and a byte-303 change; the web `src/features/short/venueGuard.test.ts` passed 2/2. Program `TraderSwept` destination `3` is `SWEEP_TO_LENDERS` in `programs/shortwin/src/events.rs`; the indexer's existing `a_dust_flush_sweep_credits_lenders_instead_of_protocol_fees` fixture covers it, but that fixture was **not rerun** in this staging step. |

Local files are under `/home/todor_dev/.local/state/shortwin-devnet-1.0.3/`: `config/indexer-ingest.toml`, `config/indexer-api.toml`, `config/keeper-staged.toml`, `secrets/`, `logs/`, and private `start-db.sh` / `stop-db.sh`. The selected keeper config has an empty market allowlist, `dry_run=true`, and `coverage_pause.enabled=false`. It contains no fee-payer or coverage key. The release keeper parsed this config when invoked with a deliberately nonexistent key-file path and exited at that expected file-read error, before any chain access; this is a config-acceptance check, not a keeper run. The database stop/start scripts were exercised, and the API role still read all nine migrations after restart. PostgreSQL was left running on loopback for the next local step. No Solana signing key was read or written, and no transaction was signed or sent.

## Backfill boundary and remaining operation

Public devnet `getSignaturesForAddress(5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK)` returned the finalized deployment signature `2twpM5sUnQeAJLVTeKo9gHeDPfpU3iNitPZNg9UVm7At6zMyQCn6DATh9azQRk9rd1DzCi315W8HKZLbDpGhVm4j` at slot **513919880** as the program address's first signature. The indexer's [`streams`](../../services/indexer/src/ingest/rpc.rs) function (around line 615) also registers the configured venue ProgramData addresses. Its `BackfillSource::list` (around line 915) applies the one [`backfill.until_signature`](../../services/indexer/src/ingest/config.rs) value to **every** stream; [`list_signatures`](../../services/indexer/src/ingest/rpc.rs) describes both bounds as exclusive (around line 729). The deployment signature was therefore a verified boundary for the **program-address stream only**. At the initial staging check, its behavior for unrelated venue streams had not been exercised, so `until_signature` was left unset and neither backfill nor the live writer was started. The subsequent seven-stream check and bounded seed are recorded below.

## Subsequent bounded backfill seed

A fresh public-devnet preflight found a finalized Pump/Pump-fees ProgramData transaction
`4fWEbr1hnMDXkF6jAE27vEv1ckeT1qVYsZyYWEaTayYStKhFeXwNXZjf5duGkQo9EAP2CBNGrTHK1xCDaqEheRk6`
at slot **510762112**. Used as the exclusive shared `until_signature`, it returned
exactly the finalized short.win deployment signature at slot **513919880** for
the program-address stream and zero signatures for all six venue ProgramData
streams. The same seven-stream result was rechecked immediately before the
local seed. This boundary intentionally excludes predeployment venue history.

The bound was added atomically to the private mode-`0600`
`config/indexer-ingest.toml` outside Git. The source-matched indexer binary
(SHA-256 `9fb16ff3963d2f78eff0415928c2dc9bac9130c806742ea23b24470b3d3b536a`)
ran `backfill` with a 120-second timeout and exited **0** in about five
seconds. Its report was `txs_seen=1`, `txs_written=1`, `txs_duplicate=0`,
`txs_failed=0`, `events=0`, `venue_trades=0`, `quarantined=0`,
`markets_rebuilt=0`. The private log is
`/home/todor_dev/.local/state/shortwin-devnet-1.0.3/logs/backfill-bounded-2026-10-22.log`
(mode `0600`). A subsequent loopback reader returned HTTP 200 on health,
status and empty markets. Status showed `confirmed_slot=finalized_slot=513919880`,
the program stream at that slot, and `backfill_complete=true` for all seven
streams. That flag means complete **only to the configured bound**; six venue
streams had no post-bound transaction. The reader was stopped. No live writer,
rollups or keeper was started by this backfill step, and no transaction was
signed or sent.

## Subsequent brief live-ingest smoke

The source-matched indexer `live` process was run twice briefly against the
private ingest config and loopback database. The retained rerun has a
machine-readable mode-`0600` receipt at
`/home/todor_dev/.local/state/shortwin-devnet-1.0.3/logs/live-retained-20261022T154420Z-634111.json`.
It records UTC start/end, **15.01 seconds** of operation, the process alive
after four seconds with six established TCP connections, explicit TERM and
return code `-15` (shell equivalent `143`), plus the binary hash, sanitized
config paths, private log SHA-256 and post-run reader status. The live log has
**zero WARN, ERROR, 429 or retry entries**. This is an intentional bounded
stop, not a graceful-shutdown assertion.

The follow-up loopback reader returned HTTP 200 on health, status and empty
markets. Status showed `confirmed_slot=513919880`,
`finalized_slot=513933760`, `missing=0`, `quarantined=0`,
`layout_warnings=0`, `logs_truncated=0`, the program cursor at the
deployment slot, and no market. The advanced finalized slot demonstrates
that finality polling ran during this short smoke. Both indexer processes
were stopped; no writer or port `8080` listener remained. The reader's
empty `trusted_proxies` warning is expected for the direct loopback setup.
This checks startup, connectivity and finality for an uninitialized program;
it does not establish sustained ingest, market events, rollups or alerting.

## Keeper startup boundary

A subsequent source inspection stopped the planned local keeper smoke **before
launch**. The staged `keeper-staged.toml` still has `dry_run=true`, an empty
market allowlist and `coverage_pause.enabled=false`, but its default oracle
and venue-watch duties remain enabled. In
[`Keeper::simulate_signed`](../../services/keeper/src/keeper.rs),
`tx::build` creates a signed transaction for simulation before the `dry_run`
guard prevents `chain.send`. Thus `dry_run` is not an unsigned startup mode.
The staged keeper key was not opened, no keeper process ran, and no
transaction was signed or sent. A later keeper rehearsal needs a concrete
reviewed transaction scope and the `solana-dev` signing approval gate.

The next market-phase work remains: settle a compatible governance/coverage authority layout and venue/oracle evidence, select a real market and capital ceiling, fund the separately staged keeper and smoke identities, prove ALT selection if v0 fallback needs it, run and monitor one sustained live writer and rollups, demonstrate fresh reader/UI results with real market data, exercise keeper coverage and zero-reward exits, and verify an actual alert recipient and pause drill. The bounded deployment-only backfill and brief live smoke do not establish those market behaviors. Keep public trading and the stable 1.0.3 tag held until those receipts and the broader release gates are complete.

## 2026-10-23 unsigned keeper observation

The keeper binary now has a separate `--observe` path. It accepts `--rpc`, `--metrics`, `--tick-ms`, and `--once`, but rejects fee-payer and coverage keypairs, the active keeper configuration, and the discovery endpoint. It returns before constructing `Keeper` or loading any signer. Each poll uses only `clock`, account reads and, after initialization, program-account discovery to report whether `GlobalConfig`, `VenueGuard`, and `OracleCache` exist, plus market and open-position counts. It does **not** crank prices, liquidate, settle, pause opens, simulate a signed transaction, or provide coverage.

`cargo test --manifest-path services/Cargo.toml -p shortwin-keeper --bin shortwin-keeper --locked` passed three focused tests, including an invalid-RPC one-shot that must return an error. The rebuilt binary's one-shot call to `http://127.0.0.1:0` exited **1** with a connection error; `--rpc https://api.devnet.solana.com --observe --once` exited **0** at slot **514041563** and found no configuration, guard, oracle cache, markets or positions. The read-only binary (SHA-256 `da5ee5f33666f5764a9d10e69aa35ddca0d69bf936a7e5a6ef2d2704dee3be9a`) is installed outside Git at `/home/todor_dev/.local/state/shortwin-devnet-1.0.3/bin/shortwin-keeper-observe`. The enabled `shortwin-devnet-keeper-observe.service` user unit calls it every **30 seconds**, without a keypair; user linger is enabled. `systemctl --user is-active` returned `active`, and `http://127.0.0.1:9464/metrics` returned `shortwin_keeper_observe_mode 1`, slot **514041577**, and zero for the three initialization states and both counts. Logs are in `journalctl --user -u shortwin-devnet-keeper-observe.service`. The metrics listener binds loopback only.

This observation mode is safe to leave running while the program is uninitialized. Starting the active keeper later still requires a funded hot fee payer, reviewed transaction scope and the signing approval gate. An initialized trading market additionally needs the real coverage/exit behavior and alert drill described above.
