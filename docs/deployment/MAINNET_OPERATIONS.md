# Capped mainnet operations — short.win 1.0.3

Owner decision and research date: **2026-10-27 (Europe/Kyiv)**.

The selected sequence is **capped mainnet before an external audit and a funded bug
bounty**, with protocol fees accumulated toward both afterward. The binding release
policy, cap definition, remaining gates and tag procedure are in
[MAINNET-1.0.3](../release/MAINNET-1.0.3.md). This runbook does not claim an audit,
a funded bounty, deployed mainnet program, configured VPS or successful operating
drill. All operating evidence below is **pending** until the actual operator
records it. The existing reviewed per-market borrowing limits are the selected
caps; deposits and the number of permissionlessly created markets are uncapped.
A keeper allowlist or website limit cannot make those two quantities capped.

## Host selection and budget

For a first production host, choose **DigitalOcean Frankfurt (`fra1`), General
Purpose, 4 dedicated vCPUs, 16 GiB RAM**, Ubuntu 24.04 LTS x86-64. The current
published regular plan is **USD 126/month with 50 GiB SSD**, before taxes and
additional services. Add approximately 100–150 GiB of persistent database storage
(or select a plan with at least 160 GiB total usable SSD). Price the additional
storage and backups in the actual order; they are not included in USD 126.
The Frankfurt region is listed in the provider's current region inventory.
[DigitalOcean pricing](https://www.digitalocean.com/pricing/droplets),
[regional availability](https://docs.digitalocean.com/platform/regional-availability/).

A lower-cost pilot alternative is its **Basic 4 shared vCPU / 8 GiB / 160 GiB**
plan, currently **USD 48/month**. Shared CPU contention and the smaller memory
headroom require measured keeper tick/observation and database behavior; this is
not a claim that the cheaper plan meets the coverage duty. Do not select it only
because the paused, empty devnet process was inexpensive.

**Hetzner alternative if available:** CCX23, **4 dedicated AMD vCPU / 16 GB /
160 GB NVMe**, Nuremberg (`NBG1`), currently **EUR 85.99/month excluding IPv4 and
VAT**. Its published June 2026 adjustment supersedes older EUR 31.49 quotations.
The public product page checked for this plan labels it unavailable; confirm
account/region capacity before ordering, and use the DigitalOcean option if it
cannot be provisioned. Hetzner CCX13 is only 2 dedicated vCPU / 8 GB / 80 GB,
EUR 42.99/month excluding IPv4/VAT, rather than a 4-vCPU plan.
[Hetzner plan specifications](https://www.hetzner.com/cloud/general-purpose/),
[official current price adjustment](https://docs.hetzner.com/general/infrastructure-and-availability/price-adjustment/).
The current [cloud agreement](https://www.hetzner.com/legal/cloud-server/) prohibits
cryptocurrency mining; it does not itself state a blanket ban on this off-chain
indexer/keeper application. These services are not a validator, staking node or
mining application. Provider terms and account acceptance still apply.

The 16-GiB recommendation is an **engineering estimate, not a measured production
minimum**. The reference stack has PostgreSQL with 1 GiB shared buffers and 200
maximum connections, one live writer, a rollup worker and two API processes,
plus the keeper and host monitoring. The indexer design estimates roughly
86,400 invocation rows per covered market-day: about 2.59 million rows per market
at 30-day retention, before index and event-size overhead. Measure database growth,
query latency, memory and RPC usage with the actual covered markets. Uncapped
market creation means a permanently small host is not guaranteed sufficient.
Build images/binaries separately from active keeper operation; do not run a large
Rust build on the production host while relying on its observation timing.

## RPC service and network identity

Use a paid mainnet JSON-RPC endpoint and a matching mainnet WebSocket endpoint.
Keep an independently tested fallback provider with enough credit and send
capacity for an incident. Do not use the public shared endpoint as the sole
production dependency. Verify full `getGenesisHash` responses, including the
fallback, against mainnet:

```text
5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d
```

Helius Developer is a starting candidate, currently **USD 49/month, 10 million
credits, 50 RPC requests/s and 5 transaction sends/s**. That plan is not a capacity
approval: measure full-load keeper ticks, simulations, sends, indexer history,
program-account discovery and streamed bytes. Its detailed billing table charges
1 credit for ordinary RPC, 10 for `getProgramAccounts`, and 2 per 0.1 MB of
uncompressed WebSocket data. Just one continuous 1-credit request/s is 2.592 million
credits per 30 days. Check the detailed billing table rather than assuming RPS
limits are a monthly allowance. Set usage alerts and provision credit headroom
before positions are admitted.
[Helius plans](https://www.helius.dev/pricing),
[detailed credits](https://www.helius.dev/docs/billing/credits),
[standard WebSocket methods and endpoints](https://www.helius.dev/docs/rpc/websocket).

The existing **devnet** relay is HTTP-only, checks devnet genesis, limits itself to
10,000 requests/day and does not relay WebSockets. Its 300-second idle override
was for a paused market without positions. **Neither that relay nor its idle
profile is selected for mainnet.** Do not turn an active mainnet keeper down to
that cadence to conserve free credits.

Keeper CLI `--rpc` and optional `--discovery-rpc` remain endpoint arguments.
The 1.0.3 preparation fix redacts all observer, active-keeper and discovery startup
endpoint fields to HTTP(S) host/port context: userinfo, path, query and fragment
are omitted. Its regression captures the actual startup records. This is a narrow
startup-log correction, not a claim that arbitrary downstream errors, shell
tracing, process inspection or environment inspection can never expose a secret.

For a single-operator VPS, place the private URLs in a root/service-readable
`/etc/shortwin/keeper.env` and launch under a dedicated unprivileged account.
Restrict local log/process access, use Linux `/proc` restrictions where suitable,
and keep shell tracing off. Do not share the keeper account with interactive
users or unrelated services. A URL expanded into `--rpc` is still present in the
process arguments and visible to sufficiently privileged users. An optional
reviewed loopback-only, mainnet-aware HTTP RPC proxy can keep upstream credentials
out of those arguments; it must preserve simulation context and support real
coverage/exit traffic. A proxy is not required by this runbook, and the devnet
relay is not a production substitute. The indexer's WSS endpoint is supplied
privately in its environment. Redact credentials from retained evidence and never
put keys in Git, browser builds, support tickets or this runbook.

## Roles, files and source identity

Suggested operator layout (placeholders until provisioned):

| Role or path | Responsibility |
|---|---|
| `/srv/shortwin` | Checkout of the exact approved release revision; do not float on a branch. |
| `/opt/shortwin/bin/shortwin-keeper` | Reviewed service binary; record source revision, toolchain and SHA-256. |
| `/etc/shortwin/keeper.toml` | Private operator config from the matching example. |
| `/etc/shortwin/indexer.env` | Private RPC/WSS environment; never commit it. |
| `/etc/shortwin/keeper.env` | Private `SHORTWIN_KEEPER_RPC_URL` (and optional discovery URL), readable only by the operator/service. |
| `/etc/shortwin/secrets/keeper-payer.json` | Small hot fee payer, with no protocol authority. |
| `/etc/shortwin/secrets/coverage.json` | Separate designated coverage authority; exactly one active pauser. |
| `shortwin_ingest` PostgreSQL role | Migrations, live ingestion and rollups. |
| `shortwin_api` PostgreSQL role | Read-only API queries; 2-second statement timeout. |
| Offline governance/emergency holders | Upgrade, configuration and durable emergency-pause decisions; their signers stay off the VPS. |

Separate OS service accounts and restrictive directory/file permissions are required;
only the keeper service account may read its hot signing files. The indexer has
no chain signing wallet. Keep deployment, governance and emergency signing keys off
this host. Keep PostgreSQL, API upstream ports and keeper metrics private; expose
only SSH from approved management addresses and HTTPS/WSS through a TLS reverse
proxy. Use SSH keys, firewall rules, OS security updates and bounded log retention.
Do not expose Docker's control socket or the database port publicly.

Use the pinned repository toolchain and locked dependencies on a build host:

```sh
cargo build --manifest-path services/Cargo.toml --release --locked \
  -p shortwin-keeper -p shortwin-indexer
sha256sum services/target/release/shortwin-keeper \
  services/target/release/shortwin-indexer
```

These build commands and the CLI below match `services/keeper/src/main.rs` and
`services/indexer/src/main.rs`; no build or mainnet operation was performed by
writing this document. Retain the actual service hashes and container image IDs
separately from the program ELF hash. The on-chain frozen fuzz does not test an
arbitrary new service build. Source changes require their applicable CI/review.

## Keeper profile and start sequence

Copy the matching `services/keeper/keeper.example.toml` to the private operator
config, then explicitly review at least:

```toml
# Select the exact markets for which this keeper promises funded coverage.
# Add every admitted market before allowing positions there. This is an
# operator responsibility, not an on-chain permissionless-market allowlist.
markets = { only = ["<MAINNET_MARKET_PUBLIC_KEY>"] }
dry_run = false
tx_format = "auto"
lookup_tables = ["<VERIFIED_MAINNET_PROTOCOL_ALT>"]

[budget]
global_daily_lamports = 1000000000
var_floor_lamports = 0
reserve_margin_bps = 10000
tail_share_bps = 2000
priority_ceiling = 100000
exit_priority_ceiling = 100000
priority_floor = 0

[coverage_pause]
enabled = true
authority_keypair = "/etc/shortwin/secrets/coverage.json"
```

This fragment is a planning profile; merge these keys into the complete matching
example. The zero VaR floor covers small pilot positions as covered markets.
Retain the normal oracle, venue-watch, sweep, graduation, pool-selection and
coverage defaults applicable to supported mainnet venues. Do not copy devnet
oracle exceptions, unavailable-venue warnings or disabled discovery into mainnet.
Determine the actual mainnet ALT addresses and check them; devnet tables do not
exist on mainnet merely because the same program ID is used. The public coverage
key must be installed in the actual `GlobalConfig` before relying on automatic
pause. Keep all unstaffed/unfunded admitted activity durably paused through the
reviewed emergency controls; a self-clearing coverage pause is not a launch hold.

After program/config/venue initialization has been independently verified, an
operator can first inspect readiness without a signer. Run this from the protected
operator shell with tracing disabled; the operator supplies the private environment
file containing `SHORTWIN_KEEPER_RPC_URL=<PRIVATE_MAINNET_HTTPS_RPC>`:

```sh
set +x
set -a
. /etc/shortwin/keeper.env
set +a
/opt/shortwin/bin/shortwin-keeper \
  --rpc "$SHORTWIN_KEEPER_RPC_URL" --observe --once
```

This mode sends no keeper transactions and provides no liquidation/coverage duty.
Once the actual payer is funded, authorities are installed and the start is
approved, supervise the active keeper with systemd, using a private
`EnvironmentFile=/etc/shortwin/keeper.env` and the corresponding command:

```sh
/opt/shortwin/bin/shortwin-keeper \
  --rpc "$SHORTWIN_KEEPER_RPC_URL" \
  --keypair /etc/shortwin/secrets/keeper-payer.json \
  --config /etc/shortwin/keeper.toml \
  --coverage-keypair /etc/shortwin/secrets/coverage.json \
  --tick-ms 400 --metrics 127.0.0.1:9464
```

Use `Restart=on-failure`, a dedicated user, a restrictive umask, restart-rate limits
and resource isolation so database/backfill load cannot starve the keeper. Verify
restart recovery and coverage, not just an `active` service state. `dry_run=true`
is not equivalent to `--observe`: the configured keeper may sign simulations.
Do not start it with a signer merely to inspect readiness.

## Indexer database, migration, backfill and API

The checked-in reference deployment is
[`services/indexer/deploy`](../../services/indexer/deploy/docker-compose.yml).
It pins PostgreSQL and Docker build bases by digest, supplies separate ingest/API
roles and binds the API upstreams to `127.0.0.1:8080–8081`. The current image alias
is `shortwin-indexer:latest` and `pull_policy` requests a build: build from the exact
reviewed checkout before keeper activity, retain the resulting image ID/digest,
and do not treat the word `latest` as provenance. Pin the operational image to
that immutable identity for restarts/rollbacks and rehearse the exact Compose
version before production; the reference stack itself is not a deployment receipt.

The following are operator setup commands, not actions already executed:

```sh
cd /srv/shortwin/services/indexer/deploy
umask 077
mkdir -p secrets
for role in admin ingest api; do
  test -s "secrets/pg_$role" || openssl rand -hex 24 > "secrets/pg_$role"
done
# The operator creates /etc/shortwin/indexer.env privately with:
# SHORTWIN_RPC_URL=<PRIVATE_MAINNET_HTTPS_RPC>
# SHORTWIN_WS_URL=<PRIVATE_MAINNET_WSS_RPC>
set -a
. /etc/shortwin/indexer.env
set +a
docker compose config --quiet
docker compose build
docker compose up -d postgres migrate
# Review and set the exact history boundary before this first backfill.
docker compose run --rm ingest backfill
docker compose up -d
```

Do not regenerate database passwords after a persistent volume has initialized;
the init script is for a new database. Do not delete volumes to resolve a role or
migration error. Confirm migrations finished successfully before live ingestion.
Retain backfill start/end, source revision, history boundary and any missing or
quarantined transactions. The shared `until_signature` bound affects all configured
streams and is exclusive; do not blindly copy a devnet signature or omit relevant
mainnet history. A bounded rehearsal/backfill is not proof of complete ingest.

Keep exactly **one** `live` ingest writer. `rollups` is a separate process; the two
`serve` replicas use the read-only role. Retain default 30-day invocation retention
and measure growth before lowering it: admin/listing/transparency evidence has
separate retention rules. If using a managed database, the writer's advisory lock
and each API's `LISTEN` connection require session-mode connections, not a
transaction-mode pooler.

Set the actual frontend CORS origin, public HTTPS/WSS names and trusted proxy
addresses. The reference `172.30.0.1` gateway applies only to its documented Docker
network; a different proxy topology needs its actual source address. Rehearse
`/v1/ws`, proxy disconnect/reconnect, per-client limits and rate limiting. Verify
304-byte full-digest guard clients and sweep destination `3` using the deployed
artifact identities and controlled first-use receipts, not just a source-file match.

```sh
curl --fail --silent --show-error http://127.0.0.1:8080/v1/health
curl --fail --silent --show-error http://127.0.0.1:8080/v1/status
curl --fail --silent --show-error http://127.0.0.1:9464/metrics
```

Healthy HTTP responses do not prove current chain data or a covered position.
Compare indexed slot/finality and known on-chain market/position state, and retain
actual keeper covered-seconds, required-observation misses and exit receipts.

## Funding and fee separation

Fund the keeper **before** admitting real positions; projected protocol fees do
not pay today's transactions. Keep a named person responsible for top-ups and
an independently observed balance. Use at least **seven days of measured operating
burn plus an explicit exit, account-rent and incident reserve** as a planning
target; alert before the balance falls below three measured days and top up to
the target after review. Record actual chosen SOL thresholds and ownership.
The 1-SOL/day observation budget is **not a total spend cap**: exit-critical
observation overdrafts, exits, oracle work, graduation, account creation and
coverage pauses can spend outside it. A temporary low balance or budget cannot
be repaired by relying on profitable liquidation rewards; eligible exits may
pay zero keeper reward.

The owner's later audit/bounty funding comes from **actual protocol-owned fees**.
Keep a public accounting record of received fees, operating expenses and the
amount reserved toward each security engagement. Do not count lender deposits,
trader margin, insurance/claim pots, unsettled claims or third-party cashback as
spendable protocol revenue. No bounty reward or audit appointment may be described
as funded/commissioned until its real funds and terms exist. Revenue is uncertain;
this policy does not guarantee when either engagement becomes affordable.

## Monitoring, backup and incident drill

Use an external watchdog outside this VPS/provider failure domain. Scrape private
metrics locally and export only needed authenticated/sanitized signals. Before
new positions, prove alert delivery to a named, reachable operator and the
Emergency holders. Watch at least:

- Keeper process heartbeat and increasing `shortwin_keeper_ticks_total`; required
  observation misses, coverage gaps, rejected charges, degraded/coverage-pause
  states and failure/refusal logs, not only process uptime.
- `Degraded`, `BudgetExhausted`, `ExitOverdraft`, `CoveragePauseUnavailable`, repeated
  `CoveragePauseFailed`, foreign coverage sets/clears and `OpenWhileDegraded`.
- Fee-payer balance, actual daily SOL burn, RPC credits/RPS/send headroom, 429s,
  context-slot lag, venue deploy/fee/authority changes and oracle age/refusals.
- Indexer `/v1/status` lag over 60 s, missing/unresolved signatures, quarantine or
  layout warnings, finality backlog, rollback behavior and disconnected WSS.
- Free disk space (warning below 25%), database/log growth, available memory,
  restart/OOM events, CPU contention and backup age/restore failures.

Perform the established coverage-pause/recovery and emergency escalation drill
in the appropriate controlled rehearsal, including a real notification receipt.
The documented ordinary recovery is 600 seconds of restored coverage; new opens
must remain paused during degraded/unfunded operation while exits continue.
Do not stop a funded keeper with live positions merely to demonstrate a mainnet
outage. A second funded standby in a separate failure domain improves resilience;
only one instance should carry the designated pauser role.

Keep daily **encrypted off-host PostgreSQL backups**, retained configuration and
migration/image identities, and test restore to a separate database before launch.
Choose and record an achievable RPO/RTO; point-in-time recovery/WAL retention is
needed if losing a day's database changes is unacceptable. Snapshots supplement
database-consistent backups. Hetzner's daily backup feature keeps seven rotating
copies and excludes attached Volumes, so a database volume needs its own backup.
[Provider backup scope](https://docs.hetzner.com/cloud/servers/backups-snapshots/overview/).
Retain secrets separately and privately; do not put hot signers in ordinary public
backup or evidence bundles. Test a restore and cursor catch-up without starting a
second production writer against the live database.

## Public operating evidence required before opening

All rows are **pending** until the real deployment supplies timestamped receipts.
Public evidence contains public keys and redacted endpoint/provider identities,
never private URLs, API keys, keypair contents or database credentials.

- Exact release/source revision, approved mainnet program/ELF hash and deployment
  identity; service SHA-256/image digests and web/client build identity.
- Mainnet genesis and current venue/header/ELF disposition, real config and
  authority public keys, multisig/timelock identities, ALT checks and the cap values.
- VPS/provider/region, funded keeper payer public key/current balance and selected
  observation/incident budget, top-up owner and reachable emergency contacts.
- Covered-market set and its relation to all existing/admitted markets; defined
  response to newly created permissionless markets and an uncovered-market alert.
- Actual 400-ms active keeper operation with a controlled open position: required
  observation coverage, known zero-reward exit path, pause/recovery and received alerts.
- Singleton writer, migrations/backfill/finality receipts, source-matched readers,
  destination-3 and full-digest guard compatibility, public API/WSS/client smoke.
- Measured RPC credit/send/latency and host/database headroom, funded fallback RPC,
  external watchdog receipt, backup restore and incident operator handoff.

These are operating requirements for the chosen early mainnet release, not a
reintroduction of an external-audit/bounty prerequisite. Provisioning, purchases,
fund transfers, signing and deployment require the actual operator's separate
execution; this document performs none of them.
