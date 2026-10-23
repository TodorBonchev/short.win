# Local devnet operator plan — 2026-10-22

**The program-only devnet deployment and local service staging are complete; operator and market readiness remain pending and no new market is approved.** These are selected rehearsal defaults. The [staging record](devnet-local-service-staging-2026-10-22.md) verifies source-matched binaries, a loopback database and empty-reader/web previews; it does not establish a running keeper, chain ingest, alert delivery or a functional market. No service key, fund transfer or initialization was part of that initial staging; four separate devnet service/smoke keys were generated afterward as described below. The payer `3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14` held **16.638061120 devnet SOL** after the approved deployment. The [readiness record](DEVNET_1_0_3_READINESS_2026_10_22.md) and [deployment receipt](DEVNET_1_0_3_DEPLOYMENT_2026_10_22.md) distinguish the earlier 25-SOL funding observation from the final balance.

This plan uses the documented [keeper startup/runbook](../../services/README.md#run), [indexer startup](../../services/README.md#indexer-m8), [keeper configuration](../../services/keeper/keeper.example.toml), [indexer configuration](../../services/indexer/indexer.example.toml) and [indexer operations §12](../design/INDEXER.md#12-deployment-and-operations). It does not establish API/WebSocket implementation compatibility or reopen that stopped review.

## Selected topology and identities

Use one operator workstation for the bounded rehearsal. Service listeners stay on loopback; chain access uses public devnet RPC `https://api.devnet.solana.com` and WS `wss://api.devnet.solana.com`. Verify devnet genesis `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG` again before any approved execution.

| Component | Selected identity and exposure |
|---|---|
| Deployment/initial authority | The deployed temporary upgrade authority is the payer above, with 16.638061120 SOL at the post-deployment check; keep it separate from service wallets. Canonical program signer remains a separate owner-held signer for `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK`. |
| Keeper | One process; a dedicated hot fee payer `35DeTJZZKgHzbLfWe9yiM1EwFfVUTcPCFSizLd8crDbc` with no protocol authority. Metrics at `127.0.0.1:9464`. No standby needed for this local rehearsal. The key exists locally but is unfunded. |
| Coverage pauser | A second, distinct keeper key `52oLAneAy9CUhtAnSjfjxMX7fqYnbCuG4Nd9hKvmKQpS`; exactly one designated pauser after its public key is installed as `coverage_authority`. Keep it separate from deployment, fee-payer and governance keys. The key exists locally but is not installed on chain. |
| PostgreSQL | PostgreSQL 17.11 database `shortwin_devnet` initialized and running at `127.0.0.1:5432`; separate private ingest/API credentials and roles verified. |
| Indexer writer | Database role `shortwin_ingest` and nine migrations verified. A [bounded deployment-only backfill and retained brief live smoke](devnet-local-service-staging-2026-10-22.md#subsequent-brief-live-ingest-smoke) succeeded; one sustained `live` writer and separate `rollups` process remain pending. No chain wallet is needed. |
| Indexer reader | Source-matched `serve` with read-only role `shortwin_api` and 2-s statement timeout passed loopback health/status smokes at `127.0.0.1:8080` before and after the bounded seed and live smoke, then stopped. It has no market data. No chain wallet is needed. |
| Browser/client | Devnet-configured web build and loopback preview at `http://127.0.0.1:4173` passed, then stopped. Reader URLs are `http://127.0.0.1:8080` and `ws://127.0.0.1:8080/v1/ws`; no live chain data or wallet trade was shown. |
| Smoke participants | Separate lender `APCRy7175QKaNK7YM8QcvxTFd7npQQPjwfKpnzqRdfN2` and borrower `5HXuXQvF2iqDqT2VjCWwk2J6gNXg5CNeob4DdabaYqBZ` test wallets, with no protocol authority. Both are unfunded. |

Use the documented database-role separation rather than the combined `run` command. A direct local reader needs no reverse proxy, load balancer, second replica or trusted-proxy entry. Do not use the reference compose gateway address in a direct-host configuration. Database passwords and all key material must remain outside Git. The four devnet-only keypair files above were generated under `/home/todor_dev/.local/state/shortwin-devnet-1.0.3/secrets/` with a `0700` directory and `0600` files; their public keys were verified with `solana-keygen pubkey`, and all are distinct from the deployment payer. No existing deployment key was opened, and no funding or transaction occurred. The keeper remains stopped with an empty market allowlist. Its `dry_run=true` setting blocks sends only after a possible signed simulation; it does not authorize an unsigned startup smoke.

## Public configuration templates

These are TOML fragments for later operator-owned files, not files written into service configuration. Unspecified values inherit the matching checked-out example's defaults. Runtime/configuration acceptance has not been exercised.

The staged keeper stays stopped with an empty allowlist, `dry_run=true`, and no coverage key loaded. As [source inspection confirmed](devnet-local-service-staging-2026-10-22.md#keeper-startup-boundary), dry-run still has a signed-simulation path:

```toml
markets = { only = [] }
dry_run = true
tx_format = "auto"
lookup_tables = []

[budget]
global_daily_lamports = 1000000000
var_floor_lamports = 0
reserve_margin_bps = 10000
tail_share_bps = 2000
priority_ceiling = 100000
exit_priority_ceiling = 100000
priority_floor = 0

[coverage_pause]
enabled = false
authority_keypair = ""

[pool_select]
enabled = false
```

For the first approved real Pump/PumpSwap smoke, replace the allowlist with **one exact market public key**, set `dry_run = false`, and enable the sole pauser with its separately held key. `var_floor_lamports = 0` deliberately gives a small smoke position the documented covered-market behavior. Retain default 400-ms ticks, 30-s idle/no-route cadence, three missed required seconds to degrade, and 600 s of restored coverage before unpausing. Keep Raydium pool selection disabled while the release's canonical Raydium identities are unavailable on devnet. Keep oracle and venue monitoring enabled. No test venue, identity substitution, Jito or Yellowstone profile is selected.

Empty lookup tables are a staging value. Before a real smoke, determine and check the protocol ALT and any per-market ALT required by the actual v0 fallback transactions; record their public addresses in `lookup_tables`, with the same public table selection in the client. Restart the keeper after extensions as documented. Do not create a table in this preparation.

Use these shared public settings in the future indexer writer and reader configurations:

```toml
[ingest]
rpc_url = "https://api.devnet.solana.com"
ws_url = "wss://api.devnet.solana.com"
program_id = "5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK"
live_source = "logs_subscribe"

[ingest.backfill]
page_limit = 1000
fetch_concurrency = 2
max_rps = 5
until_signature = "4fWEbr1hnMDXkF6jAE27vEv1ckeT1qVYsZyYWEaTayYStKhFeXwNXZjf5duGkQo9EAP2CBNGrTHK1xCDaqEheRk6"

[ingest.venue_trades]
third_party = false
```

The finalized deployment signature is a real program-address event at slot **513919880**. The [seven-stream probe and bounded seed](devnet-local-service-staging-2026-10-22.md#subsequent-bounded-backfill-seed) selected the older finalized signature above because `until_signature` is exclusive **and applies to all configured streams**, including venue ProgramData. The private ingest config contains this tested bound; the one-time backfill completed and intentionally excluded predeployment venue history. Recheck the bound and live stream cursors before future backfills. The lower backfill concurrency/rate is a rehearsal choice for the public RPC, not a measured service limit. On unresolved history/finality or rate limiting, pause the smoke and resolve the endpoint requirement; do not rotate endpoints or run an unbounded retry loop.

Writer role fragment:

```toml
[database]
url = "postgres://shortwin_ingest@127.0.0.1:5432/shortwin_devnet"
max_connections = 8
migrate_on_start = false
```

Reader role fragment:

```toml
[database]
url = "postgres://shortwin_api@127.0.0.1:5432/shortwin_devnet"
max_connections = 4
migrate_on_start = false

[api]
listen = "127.0.0.1:8080"
cors_origins = ["http://127.0.0.1:4173"]
trusted_proxies = []
statement_timeout_ms = 2000
```

The password-free URLs identify roles only; database authentication still requires provisioning. Keep existing API limits/timeouts otherwise. A future public frontend build must explicitly set `VITE_CLUSTER=devnet`, both public devnet RPC values, both actual reader URLs, explorer `https://explorer.solana.com/tx/{sig}?cluster=devnet`, and `VITE_ENABLE_TEST_VENUE=false`. These proposed localhost URLs become usable only after the compatible readers run and are checked.

## Funding, monitoring and release ownership

The release operator owns service funding, process supervision, local metric/log collection and alert delivery; name the actual recipient before a market opens. The operator must be present for the first smoke. The Emergency authority holder is the escalation owner for unavailable/failed coverage pausing and authority incidents; its actual public layout/contact remains a blocker.

- The approved upload spent **8.361938880 SOL** from the 25-SOL payer balance, leaving **16.638061120 SOL** at the post-deployment check, above the selected 5-SOL reserve. For later market operation, target **2 SOL** in the separate keeper fee payer; manually review a top-up below **1.5 SOL**, restoring at most that 2-SOL target. These are selected planning thresholds, not funded balances or authorized transfers. No automatic top-up daemon is selected.
- Keep the documented **1-SOL/UTC-day observation budget** and **100,000 µlamports/CU** observation/exit priority ceilings. The daily budget is **not a total spending cap**: exit-critical overdrafts, exits, graduation/account creation, oracle work and coverage pauses need a funded reserve. Before signing the smoke packet, account separately for seed liquidity, lender/borrower funds, rents and a finite overall capital ceiling.
- Page immediately on `Degraded`, `BudgetExhausted`, `ExitOverdraft`, `CoveragePauseUnavailable`, repeated `CoveragePauseFailed`, foreign coverage sets/clears, `OpenWhileDegraded`, actionable exit/transition/sweep refusals, venue upgrades and oracle aging/unreadability/refusals. Keep the documented automatic coverage pause/recovery behavior; a deliberate operator hold on new markets must not rely on an automatically clearing coverage flag.
- Collect keeper metrics/logs and the documented indexer status signals. Page on quarantined/layout-warning/missing events, status lag over **60 s**, unresolved finality backlog, rollback reports and WebSocket lagged closes. Confirm alert delivery and a zero-reward eligible exit before admitting ordinary test users. No monitor or alert route was installed or tested here.

## Ordered readiness gates

1. Keep the no-new-market hold. The exact program artifact, loader-only deployment, source-matched keeper/indexer builds, devnet web build, PostgreSQL roles and nine schema migrations are complete. Verify running monitoring, distinct service keys and the remaining destination-3/304-byte live compatibility before a real-market smoke. Built binaries and an empty reader smoke do not prove those services are operating.
2. The first successful deployment signature `2twpM5sUnQeAJLVTeKo9gHeDPfpU3iNitPZNg9UVm7At6zMyQCn6DATh9azQRk9rd1DzCi315W8HKZLbDpGhVm4j` finalized at slot **513919880**; the deployed identity, ELF and authority were checked. Migrations, bounded deployment-only backfill and a brief retained live connectivity/finality smoke are complete. Next run and monitor one sustained live writer plus rollups and the read-only reader, retaining successful status observations with real market data before a market smoke. Backfill/replay/rebuild must not compete with the live writer.
3. Resolve the public authority layout and the mutable devnet Squads disposition without weakening immutability checks. Provision the distinct service public keys and coverage role; verify the pauser can act and the Emergency escalation is real. A disabled-venue initialization rehearsal remains narrower than market readiness.
4. Select one actual supported Pump/PumpSwap mint/curve/canonical pool; retain scoped venue compatibility, real reserves/burned-LP checks as applicable, fresh Pyth under the existing 120-s limit, reviewed guard slots/extras, ALT decision and capital ceiling. Canonical Raydium/Jupiter scope remains disabled.
5. Prove actual keeper coverage, funded zero-reward exits, reader/UI operation, alert receipt and manual top-up ownership in the bounded smoke. Exercise the documented coverage-pause drill only in that separately approved smoke. Keep new listings/ordinary test access held until those receipts exist.

Missing inputs are actual live writer/reader and monitored keeper operation, public governance member identities and installation of the selected coverage authority, a selected market with venue/oracle evidence, ALT addresses if needed, available keeper and participant funds, a scoped capital ceiling, and a reachable alert recipient/emergency contact. The program-only deployment and local service staging close their narrow artifact and schema checks, but not these operator or market gates.

## Devnet-only direct-key initialization planner

For a bounded public-devnet trading rehearsal where the canonical devnet Squads
program is mutable, `apps/web/scripts/devnet-init-config.ts` can install **direct,
distinct devnet-only keys** instead of pretending that the production Squads
governance topology has been verified. This does not change the release program,
its 24-hour minimum config timelock, or `shortwin-gov verify`'s immutable-Squads
requirement. The direct governance and pause keys must be retained outside Git;
the normal `shortwin-gov` staging flow does not administer this layout. Use a
separate direct-key admin flow for later governance actions.

The command validates the devnet genesis, the deployed ShortWin program's
ProgramData and upgrade authority, all seven absent configuration PDAs, current
venue ProgramData slots and the exact full, fresh Pyth SOL/USD update. It builds
a 1,198-byte legacy `initialize_config` transaction with the compiled ECON
defaults, simulates it **without reading a keypair**, reports the seven rent
recipients and payer debit, and caps that debit at 0.2 devnet SOL by default.
It sets all venue bits to zero by default. Enabling Pump curve (`--venue-bits
1`) or Pump curve plus PumpSwap (`--venue-bits 3`) is an explicit operator
choice after venue, market, keeper and alert checks; enabling later through
governance takes the protocol timelock. Raydium/Jupiter bits are unavailable
through this devnet planner.

```sh
pnpm install --frozen-lockfile
pnpm --filter @shortwin/math-wasm build
cd apps/web
node scripts/devnet-init-config.ts \
  --upgrade-authority 3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14 \
  --governance REPLACE_WITH_DEVNET_GOVERNANCE_PUBLIC_KEY \
  --pause-authority REPLACE_WITH_DEVNET_PAUSE_PUBLIC_KEY \
  --coverage-authority 52oLAneAy9CUhtAnSjfjxMX7fqYnbCuG4Nd9hKvmKQpS \
  --fee-recipient REPLACE_WITH_DEVNET_FEE_RECIPIENT_PUBLIC_KEY \
  --venue-bits 0
```

The unsigned output includes `planSha256`. Only after reviewing that summary
and its successful simulation, run the same command with `--send
--expected-plan-sha256 <printed-hash> --authority-keypair
<outside-repo-devnet-upgrade-authority-keypair>`. The command then checks the
keypair's public key without printing secret bytes, runs a signed simulation,
sends once and waits for finality. If finality times out, inspect its signature
before any retry. It refuses a non-devnet genesis even when `--rpc-url`
overrides the default public endpoint.
