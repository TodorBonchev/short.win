# New pooled-version operating contract

Use the `simple/` workspace and its generated client only. The active candidate
is untagged **1.1.0**, with generated public program ID
`BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh`; its selected deployment
key stays private outside Git. It has not been deployed or tagged. Legacy keeper,
indexer projections, frontend and governance builders use different accounts and
must not operate this program. Old placeholder-ID captures, pre-identity-freeze
reports and legacy release/duration records remain historical. The
[dated final preparation record](../../docs/audit/simple-final-preparation-2026-10-29.md)
tracks the current candidate and unresolved gates.

## Local verification

```sh
python3 scripts/materialize-jupiter-probe.py
cd simple
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build-sbf --manifest-path programs/shortwin-simple/Cargo.toml --tools-version v1.57 --arch v3 --sbf-out-dir target/deploy -- --locked
cargo test --workspace --locked
python3 -m unittest discover -s services/tests -v
pnpm install --frozen-lockfile
pnpm check
pnpm build
```

Generate IDL with `anchor idl build -p shortwin_simple -o target/idl/shortwin_simple.json`, then copy it to `client/idl/shortwin_simple.json` and run `pnpm --dir client generate`. Verify source/IDL/client together; generated wire contract tests compare exact Rust bytes, including values above JavaScript's safe integer range. Use raw integer token units and lamports throughout.

## Operator integration still to commission

The initial service uses Python stdlib/SQLite and Node 24, avoiding the old Postgres, WASM math and per-venue services. It fits a small development VPS; real sizing depends on active distinct quantities and Jupiter/RPC budgets. No VPS or provider account is provisioned by this implementation, and no paid service is assumed.

1. Confirm custody of the selected private deployment key for the public program ID above, and assign governance, emergency, reporter, fee-recipient and keeper roles; those roles remain unassigned. The initializer must be authorized by the actual canonical ProgramData upgrade authority. Keep reporter and governance roles distinct. Review the exact Jupiter loader instance/digest before activation.
2. Select and review the immutable financial policy and history settings together. The service model hash and policy price_policy_hash must match byte for byte; epoch counts, timing, freshness and spread settings must also match. Example/test settings are not a signed production configuration.
3. Use the implemented revision-checked queue reconciliation with complete finalized program accounts and their same-bank clock under the shared exclusive publisher/pass lock. Retain exact-q warm-up requests, pending publication barriers and active-debt priority. A slot can retire only after debt/pending work is absent, finite references have expired in both clocks and the warm-intent rule is satisfied. Re-admission advances the generation and starts cold history; retained prior evidence remains attributable. Capacity exhaustion or provider outage is unavailable coverage, not evidence of a healthy price. Observe all required history epochs before publishing a finite reference for a new quantity.
4. Connect the generated publish/arm/force/default instructions to a bounded, funded runner. Publication uses current on-chain sequence+reporter epoch, actual RPC clock, retained local history evidence and simulation. Keeper action planning must reread the loan and immutable policy, choose the deadline/default path first, and submit a fresh simulated route for forced buyback. Do not forward Jupiter setup, cleanup or tip instructions around the protocol handler. The Node-only operator implements this bounded loop; dry-run is the default and explicit execute mode requires role keyfiles. Its funded automatic operation still needs to be commissioned and tested end to end.
5. Record the actual running client/indexer/reporter/keeper artifact digests, RPC genesis hash and program identity. Fund rent, reporter updates, keeper temporary rent, priority fees and retries with explicit budgets. Add independent heartbeat, stale-reference, queue saturation, payer-low, pending-expiry and RPC/provider outage alerts and test their delivery. Code tests do not prove these operating conditions.
6. Retain the fresh selected-ID deterministic qualification: six successful captured route lifecycles and two exact expected Scorch refusals, with actual lookup-table transaction sizes. These finite BONK/JUP/RAY and Pump curve/PumpSwap/metadata-token cases do not qualify every Jupiter route or live provider behavior. The fresh 24-hour whole-system qualification gate is still open; unfinished combined-campaign tooling was isolated after the provider stopped that work, and no new 24-hour run started. Existing duration tooling covers historical CPMM scenarios rather than the full current route set; neither it nor the pure-ledger model fills this gap. Do not substitute old duration or a short deterministic replay for missing final-runtime qualification.
7. Present the reviewed candidate commit, reproducible program hash, concrete parameter file, cost/coverage plan and operating receipts for the owner's stable-tag/deployment decision. Mainnet before external audit/bounty remains the owner's policy, with the limitations and trust terms disclosed. Never mark missing operating evidence passed.

The minimal web UI defaults to a local RPC and supports wallet review after simulation. It is a development interface, not proof of hosted mainnet availability. Do not point the legacy web release at the new program.

## CLI components

Run the trusted service from the repository checkout so its fixed script path and model settings remain attributable. `pnpm --dir client operator --help` lists the one-pass runner, and `pnpm --dir client server --help` lists the optional loopback HTTP bridge. Both need `SOLANA_RPC_URL`; the bridge does not sign. The operator defaults to dry-run and requires an explicit execute flag and matching role keyfiles for actual submission. Do not pass seed phrases, API keys or wallet bytes in command arguments or commit them.

The browser's service URL defaults to `http://127.0.0.1:8787`; Vite's development origin is `http://127.0.0.1:5173`. For a hosted deployment use HTTPS and the configured exact frontend origin, keep RPC/provider credentials server-side, and put any process supervisor/rotation/alert configuration under the operator's concrete deployment review. Do not start overlapping operator passes. An inactive one-pass timer must allow each prior pass to finish.

The Python `capacity` command reports a spacing-only request-budget bound and explicitly leaves measured coverage pending. Free API access can support only a small number of distinct quantities at a time. The example uses one queued exact quantity and conservative spacing; keyless access requires slower spacing. Increasing queue length alone does not increase quote capacity. Public requests are rate/size bounded, active debt is prioritized, and missing coverage yields Unknown. Automatic admission means no manual per-token approval, not unlimited instant quote availability.

## Long-run qualification remains open

The existing actual-SBF `program_fuzz` example has bounded and duration modes for
its historical CPMM scenarios. Its scenario and operation counts cannot be
presented as six-route current coverage. Fresh deterministic route qualification
is recorded separately for the selected public ID, and does not claim a
continuous campaign or history-service capacity.

Unfinished combined-campaign tooling was isolated after the provider stopped
that work. It has not supplied qualifying evidence, no new 24-hour run has
started, and this guide does not authorize resuming it. The unresolved long-run
gate remains open in the [final preparation record](../../docs/audit/simple-final-preparation-2026-10-29.md).
Retain partial/error evidence and its actual scope; no legacy run, checkpoint or
bounded smoke closes the gate.

## Initial production proposal and on-demand cost bounds (2026-10-29)

The reviewable financial, service, role and payer manifests are in
[`config/`](../config/README.md). The owner selected a free Jupiter key and role
setup. The generated public program ID is assigned for review, while governance,
emergency, reporter, fee-recipient and keeper identities remain unassigned. Do
not substitute test public keys or the historical program placeholder. Both
proposed policy hashes now bind MODEL_VERSION3 and require fresh v3 history.
Verify the financial policy and actual service model together before constructing
initialization instructions. These manifests remain non-executable proposals;
funding, measured real free-account coverage and delivered alerts are pending.

Private exact-q observations remain off chain while an unarmed loan is healthy.
A public warm request records a publication intent; it expires after
`epoch_seconds * (history_epochs + 2) + reference_max_age_seconds`. A usable
confirmed reference satisfies it without a new transaction. Otherwise one finite
candidate is published once and fulfillment is stored after confirmation.
Publishing is also needed to arm/confirm adverse prices, clear a recovered arm, or
attempt maturity buyback. Unknown observations confer no eligibility and do not
cause periodic chain updates. A newer healthy or Unknown private observation
vetoes this operator's actions based on an older adverse on-chain snapshot. Other
callers remain governed by the program's explicit reference-freshness window.
Each priced action
still rereads the confirmed reference and loan; local candidates never stand in
for a confirmed publication.

The queue's generation/retirement rules and persistent storage pairing are
implemented and covered by focused tests. Each operator pass checks the history
UUID and immutable program/operator-store/path binding against the operator store
and the separate `<history>.publisher-identity.sqlite` companion anchor. The
`<history>.publisher-lock.sqlite` companion stays exclusively locked for the whole
pass. Once any history UUID has been recorded, a missing/replaced file, changed
path, corrupt identity or mismatched binding intentionally fails the whole pass,
including default work, until reviewed recovery. Do not recreate stores to
recover capacity or clear a budget.

Defaults remain independent of price configuration, provider/API availability,
Unknown prices and price pause. Only a fresh first boot with no history UUID ever
recorded permits default-only work while history is absent or its service is
unavailable, under the same operator store and companion anchor. It cannot
publish, retire history or perform a price-dependent action before later initial
history binding. This exception does not permit deletion of recorded history.

For backups, stop all cooperating processes and ensure no overlapping pass or
writer. Preserve a consistent set of the history, operator, companion identity
and lock databases, including their applicable SQLite WAL/SHM/journal files;
retain raw history, generations, pending/confirmed-not-finalized publication
barriers and fulfillment records. Recovery requires review of the complete set.

Retirement frees only a monitoring slot. It does not close a reference account
or reset lifetime reference-creation reservations, daily publication fee
reservations or other financial bounds. Re-admission starts a new cold cohort
under the same persistent budget records. These protections require preserving
the complete history/operator/lock set across process restarts; code tests are
not a receipt for funded or monitored operation.

The persistent operator database reserves distinct new reference addresses before
submission. The initial allowance is four (`--max-new-reference-accounts 4`),
including unresolved outcomes and failed preparation. It also reserves publication
fees before signing against a 1,000,000-lamport **UTC calendar day** ceiling
(`--max-daily-publication-fees-lamports 1000000`). Unknown outcomes keep their
reserved budget. These local bounds require preserving the operator database;
they are not protocol-wide caps and do not authorize automatic funding. The daily
limit excludes reference-account deposits and keeper action fees. Exhaustion
keeps priced work pending and must alert; direct repayment and deterministic
default remain independent of price publication, subject to the local storage
identity checks above. Dry-run checks existing
allowances and projects additional work without consuming durable allowance.

The initial request profile tries `maxAccounts=16` then a single `maxAccounts=32`
fallback for no-route or unsupported responses, with `transactionVersion=0` and
`excludeDexes=Scorch`. This deliberately excludes a current family whose quotes
passed envelope checks but failed actual CPI execution. The ordinary RouteV2
header, amounts, quoted output, zero fees, slippage and route-step envelope are
checked before a response becomes a price observation or managed route. The
initial off-chain quote/client cap is 40 total metas (the program cap is 48),
filtering a captured 46-meta route whose actual opening transaction exceeded
1,232 bytes. This is a filter, not proof that every smaller response fits; full
transaction construction with actual lookup tables and simulation remains
required. Shared,
legacy or otherwise unsupported response shapes are unavailable. These settings
are part of the price model hash, so changes require fresh local history.

A history observation retains the first **actually quoted** budget guaranteeing
all q within the current live spread cap. A successful prior budget uses one sale
and one logical buy quote; a capped growth probe uses one extra buy. The
account-budget fallback can double those HTTP request counts to six per history
observation. This does not claim
the cheapest ask or monotonic router behavior. Execution buy searches retain their
separate bounded request budget. Measure combined history, route construction,
RPC latency and action work on the actual free account before commissioning more
than one quantity. The initial Free-key profile admits one exact quantity;
configuration alone is not evidence of measured capacity.

## Native DEX setup allowance

Some Jupiter adapters initialize or extend DEX accounts using native SOL from
the taker. The signed `route.setupLamports` is a separate native-spend allowance,
maximum 5,000,000 lamports. The payer advances that allowance plus the two scratch
account deposits within the signed `temporaryFunding` maximum. Unused allowance
and temporary scratch deposits return to the payer; actual consumed setup stays
outside WSOL sale proceeds, opening notional, margin, collateral, term fees and
keeper reward bases. This is a bounded spend allowance, not proof that arbitrary
route recipients use it only for refundable rent. The retained Pump captures consume
it for observed initialization/extension instructions; each receipt applies only
to its captured route and program identity.

The authority is loan-specific, so these expenses may recur for every new loan
and DEX family. Borrower wallet review must show the separate maximum before
signing. The initial keeper configuration sets `maxRouteSetupLamports=0`; the
keeper cannot silently subsidize new DEX setup, charge it to collateral or take it
from lenders. Commission a separately bounded payer subsidy before enabling a
nonzero keeper allowance, and reserve its entire maximum as potential permanent
spending in each dry-run pass. Real setup charges can exceed a small loan's keeper
reward. No profitability or self-funding claim follows from the upfront fee.

## Owner-selected financial duration campaign — 2026-10-29

The owner explicitly authorized the existing runner with separate current-route
and queue/operator gates if independent review supports it. The [scope and
preflight record](../../docs/audit/simple-financial-campaign-2026-10-29.md) supports
that choice and supersedes the earlier local combined-tooling launch hold. The
existing actual-SBF financial runner uses historical CPMM fixtures; six current
positive/two negative routes and service/operator checks remain separately named
bounded evidence. No continuous Pump/queue duration or live provider history is
claimed. Keep stopped combined work isolated. Freeze the manifest and verify host
identity before starting; record actual launch and independently verify >=24h
exit/duration/coverage/zero-error/frozen-hash evidence before marking this gate done.
