# Trusted price observations

This Python 3 standard-library service implements the new pooled version's local
price-history component. It does not hold a wallet, sign references, submit a
transaction, purchase a provider plan, or prove funded operation. The example
settings are functional fixtures for review, not approved mainnet settings.

`src/shortwin_price_service.py` stores exact `(market, immutable policy, raw token
quantity)` observations in SQLite with WAL and full synchronous commits. Amounts
stay integers, including quantities above SQLite's signed 64-bit integer range.
Market and mint identity must come from the new program's indexed accounts. No
interpolation between quantities or between different policy identities occurs.

Jupiter's current [build endpoint](https://developers.jup.ag/docs/api-reference/swap/build)
returns raw route instructions. The service uses ExactIn sale quotes and a
bounded search over SOL input budgets for a buy quote whose guaranteed output
threshold covers the full token debt. It never requests ExactOut or interprets
expected output as guaranteed output. It retains the successful sale and buy
response bodies and their canonical JSON SHA256 hashes. A tested buy budget is
an upper bound from that response; no globally cheapest route is claimed.
Each logical quote first requests a Jupiter routing-account budget of 16, then
tries 32 once if no supported route is returned, and excludes Scorch, whose captured ordinary-header quotes failed actual protocol CPI replay.
It requests transaction version 0. This differs from the
48-total-account protocol/client envelope: Jupiter's `maxAccounts` parameter is
not a guarantee of total returned account count. The initial quote/client profile rejects responses above 40 total metas or 768
instruction-data bytes; the program itself retains its 48-meta limit. This
filters the observed 46-meta route that exceeded a real 1,232-byte transaction,
but total-meta limits do not prove packet fit. Actual lookup-table resolution and
full protocol transaction simulation remain required. The request budget, total caps,
program identity, swap mode, transaction version, excluded DEX set and retained-ask
algorithm and deterministic fallback are part of the model hash. Ordinary RouteV2 scalar headers and amounts,
zero route fees, ceil-rounded threshold and step/data limits must match; shared or
other unsupported header shapes remain Unknown.

Completed history is sampled off-chain evidence trusted to this operator. For H
completed epochs immediately preceding the source epoch, the service requires
positive observations, epoch-boundary coverage, and no sampling gap above the
configured bound. A missing quote, stale clock, excessive spread, Unknown
observation or missing epoch produces an Unknown reference. No observation is
invented during an outage. If RPC cannot provide a usable source clock, the
tick stops and previously published references expire. The service cannot prove
what prices did between observations.

The immutable policy's `price_policy_hash` must match the `model-hash` command,
and its history, freshness and spread fields must match these settings. Changing
the settings requires a new database and fresh history. The reporter publisher
must independently read current Config, immutable Policy, market/mint bindings,
reference sequence and chain clock before constructing a signed publication.
JSON output is a candidate for review; it is not evidence that publication took
place. `max_reference_rent` is intentionally supplied by the signing SDK.

Run from `simple/`, for example:

```sh
python3 services/src/shortwin_price_service.py --settings services/settings.example.json model-hash
python3 services/src/shortwin_price_service.py --settings services/settings.example.json capacity
python3 services/src/shortwin_price_service.py --settings operator-settings.json check-policy decoded-policy.json
python3 services/src/shortwin_price_service.py --settings operator-settings.json --database /data/prices.sqlite3 enqueue requested-quantities.json
python3 services/src/shortwin_price_service.py --settings operator-settings.json --database /data/prices.sqlite3 bind-operator operator-binding.json
python3 services/src/shortwin_price_service.py --settings operator-settings.json --database /data/prices.sqlite3 queue-state
python3 services/src/shortwin_price_service.py --settings operator-settings.json --database /data/prices.sqlite3 reconcile-queue trusted-finalized-queue-snapshot.json
python3 services/src/shortwin_price_service.py --settings operator-settings.json --database /data/prices.sqlite3 plan
python3 services/src/shortwin_price_service.py --settings operator-settings.json --database /data/prices.sqlite3 tick --taker PUBLIC_KEY
python3 services/src/shortwin_price_service.py --settings operator-settings.json --database /data/prices.sqlite3 reference job.json --onchain-sequence 0 --current-slot SLOT --current-ts UNIX_SECONDS
python3 services/src/shortwin_price_service.py --settings operator-settings.json --database /data/prices.sqlite3 build-buy job.json --taker EXECUTION_AUTHORITY --maximum-input SIGNED_SOL_LIMIT
python3 -m unittest discover -s services/tests -v
```

`enqueue` takes a bounded JSON list with `market`, `policy`, `mint`, `quantity`
and optional `active_loan`. It never evicts an existing identity, including an
inactive warm-up. Legacy `sync-loans` can only add/retain active priority; a bare
list cannot prove debt disappeared and cannot demote or retire anything.

Each history database has a durable random `history_instance_id`. Before an
operator pass, its trusted local `bind-operator` call associates that history
with the deployed `program_id`, the operator store's durable `operator_store_id`
UUID and its exact absolute normalized `operator_database` path. The schema-1
payload contains exactly those four fields. The first call writes an immutable
binding; an exact duplicate returns the same identity and binding, while any
changed program, UUID or path fails without overwriting it. No binding operation
resets queue generations, reference creation allowances or publication budgets.

`queue-state` and `bind-operator` expose `history_instance_id` and
`operator_binding` (`null` in queue state before the first binding). Each operator
pass must compare identities and database paths with its separate persistent
`<history>.publisher-identity.sqlite` companion anchor, while
`<history>.publisher-lock.sqlite` stays exclusively locked for the whole pass.
A replacement history has a different UUID even with the same supplied operator
binding; this local command alone cannot recognize an absent external anchor.
After any history UUID is recorded, lost/replaced or corrupt storage and
mismatched bindings intentionally fail the whole pass, including defaults, until
reviewed recovery. Never automatically re-bind or restore fresh financial
allowances. UUIDs identify storage instances; they are neither signing keys nor
a cryptographic proof of backup continuity.

Defaults are independent of price configuration, provider/API availability,
Unknown observations and price pause. Only a fresh first boot with no history
UUID ever recorded permits default-only work before later initial history
binding under the same operator store and companion anchor. It cannot publish,
retire history or perform price-dependent actions, and it does not permit
deletion of recorded history. Stop all cooperating processes and exclude
overlapping passes/writers for a consistent backup of history, operator,
companion identity and lock databases with applicable SQLite WAL/SHM/journal
files and pending publication/fulfillment state. Restore the complete reviewed
set rather than individual replacement files.

The v3 `queue-state` command also returns the durable queue revision, last
reconciled context slot and current jobs, including generation and
publication-request time.
The operator then obtains a complete **finalized** program-account snapshot and
its clock at exactly that account context slot. Under its common exclusive
publisher/pass lock it calls `reconcile-queue` with this trusted envelope:

```json
{
  "schema": 1, "complete": true, "commitment": "finalized", "publisher_lock": true,
  "context_slot": "2000", "clock_slot": "2000", "clock_ts": 1793220000,
  "expected_queue_revision": "12",
  "active_jobs": [], "references": [], "pending_publications": [], "fulfilled_requests": []
}
```

This is a private operator contract, not a cryptographic RPC proof or public API.
`active_jobs` contains the complete deduplicated outstanding-debt identities.
`references` contains every finite on-chain reference as
`{job, source_slot, source_ts, max_age_seconds, max_age_slots}` using its immutable
policy limits. `pending_publications` contains deduplicated job identities for
all unresolved publication outcomes, including confirmed publications not yet
covered by the finalized account snapshot. Every nested `job` has the same
market/policy/mint/quantity fields as `enqueue`. `fulfilled_requests` contains
`{job, generation, requested_at}` acknowledgments bound to the admission cohort.
The operator must share its publisher lock across all processes using that role;
claiming `publisher_lock: true` cannot establish that lock by itself.

A single `BEGIN IMMEDIATE` transaction checks the queue revision and monotonic
finalized context/clock before changing scheduling. Stale, incomplete, conflicting
same-slot or raced snapshots fail atomically. Only an exact same-payload replay
with an unchanged post-apply queue revision is idempotent. A clock from a later bank is refused because it could expire a
reference after the account snapshot while missing an intervening new loan.

An identity retires only after complete finalized state proves no debt, no pending
publication, and every finite reference has expired in **both** slot and time
limits at that same bank. An unfulfilled warm intent additionally retains the
slot for `(H + 2) * epoch_seconds + reference_max_age_seconds`; TTL never overrides
debt, reference or publication protections. A full protected queue requires an
operating-capacity decision instead of silent eviction. Retirement frees only a
monitoring slot. It does not close a reference account or reset the operator's
four-account financial allowance or daily publication-fee budget.

Retirement keeps the cohort's retained raw observations and an audit digest in
SQLite. Re-admission increments its durable generation and must build fresh
history; current-generation reads and in-flight tick generation checks cannot
reuse a retired cohort. The current cohort still uses the documented rolling
history window; retired cohorts are not pruned automatically. Consistent backups,
storage growth and a reviewed archive policy remain operating requirements.
The v3 model changes the policy hash and requires a **new database**. Old model
files are rejected before schema mutation and must be preserved as old evidence.

Input paths can be `-` for JSON on standard input. `check-policy` checks decoded
camelCase policy terms and a 32-byte array or hex `pricePolicyHash`.
`build-buy` returns a freshly obtained, bounded whole-debt route candidate; it
does not reuse an old history response. Failure exits without a candidate.

`tick` needs `SOLANA_RPC_URL` from the operator environment. `JUPITER_API_KEY`
is optional. The [current platform limits](https://developers.jup.ag/changelog/developer-platform)
are one request per second with a Free key and 0.5 requests per second keyless.
Configured `request_interval_ms` is at least 1000, and keyless CLI calls require
at least 2000. A SQLite reservation serializes quote starts across `tick` and
`build-buy` processes sharing that database. Other clients, databases and shared
IP/API-key traffic still consume the same provider allowance and need a common
operator budget. A full reservation queue fails rather than waiting past
reference freshness. This needs no paid provider plan.

It performs HTTP GET quotes and read-only RPC `getSlot`/`getBlockTime` calls.
The indexer and supervisor must schedule ticks, alert on skipped/Unknown jobs,
and ensure every active loan receives service within freshness and sampling
bounds. History observations first reuse a successful previous acquisition
budget, capped to the current bid plus the reviewed spread; a retained candidate
is always a new actual full-q quote. A successful normal observation uses two
logical quotes and a capped growth probe uses at most three in total (up to six
HTTP requests when every quote needs the 32-account fallback), without assuming
monotonicity or inventing interpolated prices. Execution `build-buy` still has
its independent `max_ask_requests` budget. RPC clock reads are additional.
Jobs become due at half their maximum sample gap to leave scheduling margin.
The `capacity` command reports the spacing-only lower bound and leaves measured
latency and sustained coverage pending. The example admits one exact quantity,
targets 15-second scheduling with a 30-second maximum sample gap, and budgets up
to six HTTP requests per history observation and twelve logical buy probes per
execution search. Each execution probe can also need the account-budget fallback.
HTTP400/no-route or unsupported shape can select the alternate budget; rate limits,
authentication failures, connection errors and provider outages do not trigger it. It does not claim simultaneous monitoring of
every mint or arbitrary quantities. The keyless interval doubles request time
and can require fewer probes, a different reviewed history policy or a Free key.

The rolling database retains H plus two epochs. Archive emitted evidence before
pruning and back up the stopped, nonoverlapping complete SQLite set described above. No real provider
credentials, deployed operator configuration, funding, alert delivery, sustained
quote throughput or live mainnet compatibility was available or exercised here.

The initial Free-key production proposal and separate slower keyless fallback are
in [config/README.md](../config/README.md). Public warm requests record a dated
publication intent; authoritative debt synchronization does not refresh it.
`jobs` includes `publication_requested_at` for the Node operator's on-demand
publication decision. Healthy unarmed debt continues private sampling without
continuous reporter transactions. Active-debt capacity, public warm/rent
admission and combined history/action throughput still require commissioning.

## Queue lifecycle qualification

`queue_lifecycle_fuzz.py` exercises real SQLite admission, trusted reconciliation,
retirement, re-admission, restart and races against an independent protected-set
model. It uses synthetic clocks/identities and no network or signing. Bounded
checks do not provide 24-hour evidence or provider-throughput qualification.

```sh
python3 services/queue_lifecycle_fuzz.py --seed 103410000 --cases 64 --out target/queue-smoke-new
```

Duration mode accepts `--duration-seconds N` and an optional supervisor
`--stop-file PATH`, checked between cases. It finishes an in-flight case naturally
at the duration deadline. A supervisor stop writes readable partial evidence and
returns nonzero; it never becomes a clean completion. Source hashes, actual
operation counters, final log/summary hashes and durable completion data are
retained. Failure keeps its working SQLite state and replay seed; successful case
databases are discarded after their assertions, with seeds retained in the log.
