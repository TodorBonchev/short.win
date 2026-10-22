# Local devnet operator plan — 2026-10-22

**The program-only devnet deployment is complete; operator and market readiness remain pending and no new market is approved.** These are selected rehearsal defaults, not installed configuration or evidence of running services. This plan did not itself run a service, create a service key, transfer funds or initialize the program. The payer `3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14` held **16.638061120 devnet SOL** after the approved deployment. The [readiness record](DEVNET_1_0_3_READINESS_2026_10_22.md) and [deployment receipt](DEVNET_1_0_3_DEPLOYMENT_2026_10_22.md) distinguish the earlier 25-SOL funding observation from the final balance.

This plan uses the documented [keeper startup/runbook](../../services/README.md#run), [indexer startup](../../services/README.md#indexer-m8), [keeper configuration](../../services/keeper/keeper.example.toml), [indexer configuration](../../services/indexer/indexer.example.toml) and [indexer operations §12](../design/INDEXER.md#12-deployment-and-operations). It does not establish API/WebSocket implementation compatibility or reopen that stopped review.

## Selected topology and identities

Use one operator workstation for the bounded rehearsal. Service listeners stay on loopback; chain access uses public devnet RPC `https://api.devnet.solana.com` and WS `wss://api.devnet.solana.com`. Verify devnet genesis `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG` again before any approved execution.

| Component | Selected identity and exposure |
|---|---|
| Deployment/initial authority | The deployed temporary upgrade authority is the payer above, with 16.638061120 SOL at the post-deployment check; keep it separate from service wallets. Canonical program signer remains a separate owner-held signer for `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK`. |
| Keeper | One process; a dedicated hot fee payer with no protocol authority. Metrics at `127.0.0.1:9464`. No standby needed for this local rehearsal. |
| Coverage pauser | A second, distinct keeper key; exactly one designated pauser after its public key is installed as `coverage_authority`. Keep it separate from deployment, fee-payer and governance keys. No key has been generated. |
| PostgreSQL | Planned PostgreSQL 17.11 database `shortwin_devnet` at `127.0.0.1:5432`, session-mode connections. Installation/availability and credentials are unverified. |
| Indexer writer | Database role `shortwin_ingest`, schema owner: one-shot migrations, backfill, then exactly one `live` writer; separate `rollups` process using that role. No chain wallet is needed. |
| Indexer reader | One `serve` process using separate read-only database role `shortwin_api`, 2-s statement timeout; planned `127.0.0.1:8080`. No chain wallet is needed. |
| Browser/client | Planned UI preview at `http://127.0.0.1:4173`; anonymous local reader access, with no embedded API key. Proposed reader URLs are `http://127.0.0.1:8080` and `ws://127.0.0.1:8080/v1/ws`; neither endpoint exists as operating evidence yet. |
| Smoke participants | Separate lender and borrower test wallets, with no protocol authority; public identities and funding remain unassigned. |

Use the documented database-role separation rather than the combined `run` command. A direct local reader needs no reverse proxy, load balancer, second replica or trusted-proxy entry. Do not use the reference compose gateway address in a direct-host configuration. Database passwords and all key material must remain outside Git; none is requested or created by this document.

## Public configuration templates

These are TOML fragments for later operator-owned files, not files written into service configuration. Unspecified values inherit the matching checked-out example's defaults. Runtime/configuration acceptance has not been exercised.

The staged keeper stays stopped with an empty allowlist, simulation only, and no coverage key loaded:

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
# until_signature = "2twpM5sUnQeAJLVTeKo9gHeDPfpU3iNitPZNg9UVm7At6zMyQCn6DATh9azQRk9rd1DzCi315W8HKZLbDpGhVm4j"

[ingest.venue_trades]
third_party = false
```

The finalized deployment signature above is a real boundary candidate, recorded at slot **513919880**. Confirm the indexer's `until_signature` semantics and actual first program event before using it for backfill; the line remains commented until a source-matched indexer is configured. The lower backfill concurrency/rate is a rehearsal choice for the public RPC, not a measured service limit. On unresolved history/finality or rate limiting, pause the smoke and resolve the endpoint requirement; do not rotate endpoints or run an unbounded retry loop.

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

1. Keep the no-new-market hold. The exact program artifact and loader-only deployment packet are complete. Stage source-matched keeper, indexer and client identities, database/schema/roles and monitoring before initialization or a real-market smoke, preserving the 1.0.3 sweep-destination/304-byte VenueGuard compatibility requirement. Record exact service source refs/artifact hashes; current services are not asserted to be built or running.
2. The first successful deployment signature `2twpM5sUnQeAJLVTeKo9gHeDPfpU3iNitPZNg9UVm7At6zMyQCn6DATh9azQRk9rd1DzCi315W8HKZLbDpGhVm4j` finalized at slot **513919880**; the deployed identity, ELF and authority were checked. Next verify the indexer boundary semantics, then migrate → backfill → one live writer plus rollups and the read-only reader. Retain the compatible running identities and successful reader/status observations before initialization or a market smoke. Backfill/replay/rebuild must not compete with the live writer.
3. Resolve the public authority layout and the mutable devnet Squads disposition without weakening immutability checks. Provision the distinct service public keys and coverage role; verify the pauser can act and the Emergency escalation is real. A disabled-venue initialization rehearsal remains narrower than market readiness.
4. Select one actual supported Pump/PumpSwap mint/curve/canonical pool; retain scoped venue compatibility, real reserves/burned-LP checks as applicable, fresh Pyth under the existing 120-s limit, reviewed guard slots/extras, ALT decision and capital ceiling. Canonical Raydium/Jupiter scope remains disabled.
5. Prove actual keeper coverage, funded zero-reward exits, reader/UI operation, alert receipt and manual top-up ownership in the bounded smoke. Exercise the documented coverage-pause drill only in that separately approved smoke. Keep new listings/ordinary test access held until those receipts exist.

Missing inputs are the exact runnable **service** artifact identities, usable local PostgreSQL/roles/authentication, confirmed indexer backfill boundary semantics, public governance/coverage/smoke-wallet identities, actual selected market and venue/oracle evidence, ALT addresses if needed, available keeper funds, a signed capital ceiling, and a named alert recipient/emergency contact. The program-only deployment closes the program loader and artifact-identity checks, but none of these operator or market gates.
