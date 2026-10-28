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
python3 services/src/shortwin_price_service.py --settings operator-settings.json --database /data/prices.sqlite3 sync-loans complete-open-loan-identities.json
python3 services/src/shortwin_price_service.py --settings operator-settings.json --database /data/prices.sqlite3 plan
python3 services/src/shortwin_price_service.py --settings operator-settings.json --database /data/prices.sqlite3 tick --taker PUBLIC_KEY
python3 services/src/shortwin_price_service.py --settings operator-settings.json --database /data/prices.sqlite3 reference job.json --onchain-sequence 0 --current-slot SLOT --current-ts UNIX_SECONDS
python3 services/src/shortwin_price_service.py --settings operator-settings.json --database /data/prices.sqlite3 build-buy job.json --taker EXECUTION_AUTHORITY --maximum-input SIGNED_SOL_LIMIT
python3 -m unittest discover -s services/tests -v
```

`enqueue` takes a bounded JSON list with `market`, `policy`, `mint`, `quantity`
and optional `active_loan`. `sync-loans` accepts only a complete, trusted indexer
snapshot; the caller must de-duplicate identities. It removes priority from
resolved loans. These commands are local operator interfaces; do not expose
them as unauthenticated public APIs. Existing loan identities take priority and
can evict inactive requests when the bounded queue is full. A full queue of
existing debt requires an operator capacity decision; it cannot silently admit
another identity. A restored identity must rebuild missing history.

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
pruning and back up SQLite with a consistent backup method. No real provider
credentials, deployed operator configuration, funding, alert delivery, sustained
quote throughput or live mainnet compatibility was available or exercised here.

The initial Free-key production proposal and separate slower keyless fallback are
in [config/README.md](../config/README.md). Public warm requests record a dated
publication intent; authoritative debt synchronization does not refresh it.
`jobs` includes `publication_requested_at` for the Node operator's on-demand
publication decision. Healthy unarmed debt continues private sampling without
continuous reporter transactions. Active-debt capacity, public warm/rent
admission and combined history/action throughput still require commissioning.
