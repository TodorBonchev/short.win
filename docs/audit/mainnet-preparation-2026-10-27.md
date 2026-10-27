# Mainnet preparation engineering checkpoint — 2026-10-27

**Status: in progress, not mainnet ready.** The owner requested all necessary
preparation and approved the initial 245.20 USD/month infrastructure budget
(before tax, overages and SOL fees). The [purchase/setup proposal](../deployment/VPS_PURCHASE_AND_SETUP_2026_10_27.md)
is concrete; DigitalOcean sign-in/provisioning and real funded operation are pending.
No purchase, tag, deployment, financial/governance transaction or website publication
is claimed. First mainnet remains postponed for broad Jupiter support; external
whole-repository audit and bounty timing remains the owner's pre-audit launch choice.

## Implemented and focused checks

- Dependency-free exact-q report state, authorized publisher/domain/model/window/
  sequence validation and completed-bucket history, with a canonical 1,488-byte codec.
  Sixteen native groups include 49,152 modeled update/codec-restoration steps.
- Immutable program-owned `PriceReport` (1,547 bytes), initialized by actual configured
  governance Signer and published by an allowed runtime Signer. Five actual SBF groups
  passed; the exercised maximum publication cost was 23,401 CU. Reports do not admit
  markets or authorize health, liquidation or settlement. Configured genesis still
  needs off-chain cluster verification; no economic reconfiguration/retirement or
  authoritative outstanding-debt hooks are supplied by this wrapper.
- Fixed-q candidate search, real bounded HTTP Build adapter and read-only
  `shortwin-price-probe --config <file> --once`. Search18, HTTP16 and diagnostic8
  groups passed; the integrated keeper library passed132 tests. API candidates do
  not become simulated executions, lower financial references or no-route evidence.
  No real API key or external provider request was used in the adapter tests.
  The added shared quota file uses durable same-inode locking across cooperating
  processes, rolling one-second total/background reservations and exit headroom.
  Final integrated keeper library: **147 pass**; diagnostic binary: **8 pass**;
  all-target Clippy passes. Corrupt state, policy/clock mismatch, cancellation,
  failed transport and 429 behavior are covered, with no reservation refund and
  automatic HTTP retries explicitly disabled. API candidate/probe output explicitly
  reports `executable_minimum_verified=false`: the exact Jupiter V2 threshold
  rounding has not been established by this adapter.
  The bound applies to local reservation issuance, not provider arrival times under
  arbitrary scheduling/network delay, outside account consumers or monthly credits.
  Combined-load/credit/exit-capacity qualification remains open.
- Borrower `repay_short(max_rent)`: full net token principal, bounded full accrued
  rent, existing close/pool accounting and exact debt release without route/oracle
  reads. The full position-engine suite passed77 groups including9 repayment
  groups under SPL/Token-2022, sibling exposure, rent insufficiency and real
  transfer-fee net-credit rollback. The later combined report/repayment engine
  rerun passed **82 tests, zero failures or ignores**. Invariant facts explicitly
  verify repayment conservation, rent and net lender-vault credit; no invariant
  was disabled.
- Repay indexer/API/replay-model and nullable-P&L website compatibility passed
  focused PostgreSQL/API, library, typecheck and UI checks. The complete actual-SBF
  scenario set now replays through real PostgreSQL ingest: nine scenarios, including
  the new Repay path in the 987-transaction mock lifecycle, match chain checkpoints.
  The harness embeds its build-time IDL so copied test binaries use that exact
  catalog. Acquisition cost outside the protocol remains unknown, never invented
  zero profit.
- The borrower website supports explicit full-token repayment with a user-entered
  on-chain rent limit and ordinary transaction review/simulation. Its reader does
  not depend on a venue or quote. It preserves the exact rent bound on rebuild,
  checks simulated source/pool net-q deltas and the wallet fee before refunds, and
  discloses unknown external acquisition cost. Full frontend suite: 1,107 pass,
  one intentional live-validator skip; TypeScript and focused formatting pass.
- Regenerated IDL/client checks: 32 pass, including independent Borsh byte vectors
  for signed report timestamps/status tags, the 1,547-byte account and the exact
  repayment u64 rent bound. Existing 304-byte VenueGuard checks remain green.
- Separate quote-native financial and opening-notional rent math modules are
  implemented but not wired to financial instructions. The full math library
  passed 180 tests, strict Clippy and no_std checks. Peer review led to defensive
  rejection of impossible restored utilization-integral deltas and explicit locked
  kink/cohort binding; neither correction converts legacy lender carry or chooses
  a maturity-default policy.
- A separate test-only executor ran actual SBF full-q/refund/rollback cases and one
  historical pinned Jupiter V2 ExactIn CPMM route. This is one frozen route case,
  not a production adapter, live-price qualification or coverage of every DEX.
  Source/account/rent/CU and provider-review limits remain explicit in its README.

The initial report codec exposed a 5,312-byte SBF frame diagnostic despite the
build tool exiting0. That artifact is unqualified. The bounded-frame fix rebuilt
without diagnostics and passed actual SBF tests. A release CI log check now rejects
such diagnostics independently of build exit code; three parser regressions pass.

## Review, evidence and limits

Independent AI-agent checks found no material issue within the bounded search,
isolation-helper, report-persistence and repayment/accounting scopes reviewed.
Authorship and review scope are distinct; no external firm or accreditation is
claimed. The executor subtask was stopped by a provider cybersecurity filter during
its final follow-up. Its already completed source/results are preserved; unfinished
review work is not marked complete or silently retried. The [evidence folder](probes/mainnet-preparation-2026-10-27/)
retains the stop disposition and exact completed log bytes/hashes.

The bounded HTTP/probe and borrower repayment UI were additionally read independently
by the integrating agent. The HTTP probe exposes API-declared candidate fields only;
no JSON minimum-output claim substitutes for actual net-q simulation/execution.

All results above are incremental checkpoints, not final release qualification or
exact-head full CI. Historical green CI/fuzz belongs to its recorded
inputs. Full financial integration, current upstream compatibility, ABI/client/replay
checks, independent integration review, reproducible final bytes and fresh qualifying
24-hour zero-failure/zero-anomaly fuzz remain required. No audit finding is closed.

## Work still required

Implement and qualify the reviewed quote-native financial contract, production
isolated executor and services together. The price reference must explicitly separate
accepted bid/ask-value policy from a claim of globally cheapest cost; failed quotes
cannot manufacture default. Decide and disclose a complete lender recovery mechanism
for prolonged reporter/route outage; in-kind borrower repayment alone is insufficient.
Preserve legacy debt/exit/rent terms across any backend transition. Review exact
reporter keys and financial/settings/operating packet before deployment.

After the final runtime is frozen, run full CI/current identity checks and fresh
qualifying fuzz, then commission funded monitored operations on approved infrastructure.
Only then prepare a concrete version/tag/deployment packet for owner review.

## Current dependency identity and local build checks

The read-only full live fixture check passed at mainnet slot **462,161,911**.
Every configured program/account pin matched, including the newly qualified
Token-2022 baseline and Jupiter administrative pin. Local manifest/file/hash checks
also passed for all 346 entries. These are dated compatibility/identity observations,
not a promise that upstream programs cannot upgrade before release.

The local release/IDL build passed the added SBF-stack diagnostic check. Its
incremental release ELF SHA-256 was
`d2c486054a5bb4d5940839f051e5638cbfc88cc189b390a35fb4371486a9778f`;
this is not a reproducible final release or deployment authorization. The independent
report-account SBF suite used its separately recorded test-venue artifact.
The primary checkout's untracked `fork_seed.rs`, frozen 1.0.3 metadata anchor and
44 archived Jupiter redesign evidence checksums were reverified unchanged.

## Concrete financial decision pending

The [quote-native financial contract](../design/jupiter/QUOTE_NATIVE_FINANCIAL_CONTRACT.md)
separates bid-value health from ask-cost acquisition, locks opening-notional rent and
specifies complete candidate settings. These are proposals, not deployed defaults.
The proposed overdue-default backstop (position deadline plus a locked 24-hour grace,
remaining collateral to lenders less the capped keeper payment, zero borrower refund)
requires the owner's pending explicit choice because a healthy overdue borrower can
forfeit collateral. Its implementation/activation is not inferred from infrastructure
budget approval. Quote failures remain Unknown; no quote-only S2/S3 is introduced.

## Published checkpoint CI and lockfile correction

The engineering checkpoint was published as
`07a34d2ad8f9726184a5a534ee7cf3de948e6115` in draft PR10. Both dedicated
Jupiter-probe workflow runs passed; their test-only scope and stopped review limit
remain unchanged. The push-run artifact digest and all eight contained files were
independently hashed. Ordinary PR CI40556153080 found stale benchmark lockfiles
for the new local report-crate dependency. Only the local package/edge was added to
five bench locks; no external package/version/checksum was upgraded. Locked offline
metadata and the focused M1 check pass. The [CI/repair evidence](probes/mainnet-preparation-ci-2026-10-27/)
retains the original failure and correction. A rerun is required on the correction;
this is not a final full gate or fresh fuzz result.
