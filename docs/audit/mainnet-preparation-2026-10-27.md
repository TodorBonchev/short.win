# Mainnet preparation engineering checkpoint — 2026-10-27

**Status: in progress, not mainnet ready.** The owner requested all necessary
preparation and approved the initial 245.20 USD/month infrastructure budget
(before tax, overages and SOL fees). The [purchase/setup proposal](../deployment/VPS_PURCHASE_AND_SETUP_2026_10_27.md)
is concrete; DigitalOcean sign-in/provisioning and real funded operation are pending.
No purchase, tag, deployment, financial/governance transaction or website publication
is claimed. First mainnet remains postponed for broad Jupiter support; external
whole-repository audit and bounty timing remains the owner's pre-audit launch choice.

## Current selected history model

The owner explicitly accepts trusted off-chain price history on the VPS and our
service as authority. Use on-demand authenticated snapshots for actual actions;
continuous two-second ON-CHAIN publication is superseded. Private history is not
public timing proof, and the old publication-cost measurement is retained as
historical evidence for the rejected model. Actual per-action fees, RPC/API
usage and funded operation still need measurement.

The new snapshot codec/account/receiver, durable collector/watcher, trusted
bid/ask evaluator, distinct notional-rent persistence and quote-native repayment
bookkeeping are being qualified as a new engineering batch. No production market
adoption/opening, complete routed financial execution, cash default or funded
operation is claimed from these foundations. The owner subsequently explicitly selected overdue collateral default after the
locked 24-hour grace, including healthy borrower forfeiture. The distinct recovery
path and immutable terms are being implemented and tested; this is not activation.

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

## Concrete release packet sequence

1. Resolve the proposed financial contract before admitting quote-native debt.
   The overdue-default choice changes lender lock horizons, loss accounting and
   borrower disclosure. Final reporter public keys, policy hashes, covered lots,
   risk limits and timing settings still require the concrete owner review.
2. Finish versioned market/position persistence and actual financial/debt-lifecycle
   integration, qualified two-sided price production, production atomic execution
   and compatible keeper/readers. Host math and test-only executor results cannot
   stand in for those adapters. Keep the provider-stopped review scope disclosed;
   it has not been retried or counted as completed.
3. Select the version **before** freezing final build/fuzzer inputs. The proposed
   version is **1.1.0**, reflecting the larger redesign; it is not a tag or current
   binary version. Apply the chosen version consistently, then freeze source,
   IDL, clients, lockfiles, fixtures, commands and exact binary hashes.
4. Obtain the final full gate including selected mutation tests, reviewed current
   dependencies and reproducible Anchor/verifier bytes. Repeat the read-only live
   identity check near release; review changed upstream code before promotion.
5. Extend the harness/invariants to exercise the actual new report/admission,
   notional-rent, repayment, liquidation and chosen recovery paths before the
   campaign. Rerunning the six legacy profiles alone cannot qualify unused new
   backend paths. Run fresh qualifying all-profile fuzz on the frozen inputs:
   at least 24h, readable final summary, zero failures/anomalies/invariant failures,
   exact completion/log/artifact hashes and profile-coverage evidence. Old 1.0.3
   duration is not transferred. Any relevant input change requires new evidence.
6. Commission approved infrastructure under the owner's accounts. Record exact
   artifact/service identities, covered markets, actual funded payer balances and
   top-up policy, report freshness and combined-load/credit measurements. Verify
   isolated database restore and independently delivered outage/exit/funding alerts.
   Code tests, a purchased VM and the proposed monthly budget prove none of these.
7. Prepare the concrete unsigned deployment/configuration packet: program identity,
   canonical ELF/IDL hashes, actual authority/payer public keys, current venue pins,
   reporter policies, exact risk settings, all rent/transaction budgets and simulation
   results. Derive rent from the final artifact and live RPC; do not spend an estimate
   tied to this incremental build. Keep governance/emergency signers off the VPS.
8. After owner review and satisfied gates, use an annotated final version tag tied
   to the approved source and build receipts, then separately authorize/sign the
   deployment and configuration transactions. Begin with opens paused, verify actual
   deployed bytes/authorities/guards and commissioned readers, and enable only
   qualified covered pilot markets through the reviewed governance packet.

All launch surfaces must disclose that external audit and a funded bounty are
post-launch funding work, that operators control the in-house price authority,
and that the 25-SOL compiled opening-notional cap is per market while deposits and
market count remain uncapped. None of this sequence constitutes a purchase, tag,
deployment or financial transaction in this checkpoint.

## Historical publication cost and packet-size finding (superseded)

SDK serialization and an independent wire parser measured the proposed two-feed
v0 publication packets. Two Finite updates plus the normal compute-budget
limit/price instructions are 1,278 bytes, exceeding the 1,232-byte packet limit.
A synthetic ALT gives 1,188 bytes and establishes structural fit only. For a
continuous two-second schedule over 30 days with one signer/transaction, separate
updates cost 12.96 SOL in base fees; qualified batching would cost 6.48 SOL, before
priority fees, retries, initialization or other operations. The approved hosting/
API budget excludes these fees. The [retained measurement](probes/price-publication-cost-2026-10-27/README.md)
records exact bytes, source hashes, official fee/packet documentation and assumptions.
No real key, RPC call, signature, ALT account, simulation or transaction was used.
This remains a concrete cost/cadence/funding review prerequisite, not a new default
or approved expense.

## Follow-up CI catalog/fixture repairs

The next CI run found the fuzzer's old 30-instruction name catalog and a committed
ingest-session inventory without Repay. The statistics catalog now recognizes all
33 instructions and the Repay event, without counting any unexecuted path. The
original source/catalog assertion and new discriminator checks pass (13 library
tests); all 21 local smoke/corpus tests and strict Clippy pass against verified CI
SBF artifacts. Refreshed only the actual-SBF mock lifecycle session; the original
all 15-session PostgreSQL ingest/catalog/chain/ledger test passes, with all other
14 fixture hashes preserved. The [repair archive](probes/mainnet-preparation-ci-repair-2026-10-27/)
retains the failures, corrections and exact receipts. A fresh CI rerun remains owed.

The earlier run's independently downloaded Anchor and verifier artifacts match
byte for byte: release SHA-256
`392b6ac13811e599bb2b3c80ef175382ff7abb27bdcb4c2801b73e6eea82abc9`,
trimmed executable hash
`95c151e4081835772382dd48d0732c140eadca8f32c568991fd99d85c7dab7a2`.
Its IDL exactly matches the regenerated client. These identify the incremental
program only, not the final proposed 1.1.0 runtime or changed fuzzer/fixtures.

## Repayment form context regression

A direct rerender reproduced stale source/rent authorization and local review
state when the route reused the panel for another position. The public panel now
owns a position-keyed inner form, resetting all local repayment state together.
Same-position updates preserve explicit choices. Two regressions fail before the
fix and pass afterward; panel/useRepay10 tests, TypeScript and focused Biome pass.
No program, server, wallet/send pipeline or on-chain rent bound changed. The
[red/green receipt](probes/repay-position-switch-2026-10-27/README.md) retains
source, router evidence, exact commands and hashes. Final integration qualification
remains open.

## Prior checkpoint CI verification

Ordinary CI [40566841702](https://github.com/TodorBonchev/short.win/actions/runs/40566841702)
passed **24 jobs** at `30a2c17e5fde8c64073a93e149abb673b92584b0`, with `mutants`
and `fuzz-nightly` intentionally skipped. The focused execution probe
[40566841555](https://github.com/TodorBonchev/short.win/actions/runs/40566841555) passed.
The actual CI merge snapshot has zero file differences from that head. Independent
artifact verification matched the ZIP digests, every member hash, Anchor/verifier
release ELF raw `392b6ac13811e599bb2b3c80ef175382ff7abb27bdcb4c2801b73e6eea82abc9`,
trimmed `95c151e4081835772382dd48d0732c140eadca8f32c568991fd99d85c7dab7a2`, and
committed/fresh IDL `3fd8a6207f5bc25763c0f64c6d5e2bf8301c206aa6f952dc0e4c04da2099099c`.
This verifies the PRIOR checkpoint; it does not qualify this new trusted-history
runtime or substitute for full selected mutations and final 24-hour fuzz.


## Trusted-history / overdue-default follow-up (2026-10-28 local)

The implementation now has a distinct on-demand `PriceSnapshot` (1,624 bytes,
1,533-byte core codec), immutable per-feed producer commitments and a shared
financial policy. Prospective `QuotePolicy` initialization does not set the
quote-native backend flag or enable borrowing. The read-only reference preflight
checks actual owned accounts, canonical PDAs, quantities, policy/producer binding,
minimum sequence and actual runtime freshness. Governance/emergency can pause
trusted price actions, with governance-only clear. Legacy rent/observation entry
points reject the distinct backend instead of interpreting its state as reserves.

Typed quote-native repayment and the owner-selected overdue default are built.
Positions (348 bytes) lock the checked deadline-plus-86,400s recovery time, keeper
reward and vesting schedule. The overdue instruction uses actual Clock/unpaid debt,
requires no price/mint/token/route accounts, pays `K_s` to the keeper and `V-K_s`
to lenders, and emits `OverdueCollateralDefault`; it never claims token repayment,
S3, insolvency or fair value. The caller signs a maximum epoch-record rent bound.
Account-rent/untracked-donation refunds remain separate from borrower collateral.
A review found mutable global vesting was still being read; the fix locks the
position value and has retained red/green and actual-SBF controls for both exits.

The watcher/publisher retains off-chain H8 history and trusted two-sided API
estimates, with Unknown/gap resets, bounded requests/evidence, exact policy hashes,
explicit publication only, durable fee/burst budgets and pending-signature
reconciliation. No actual Jupiter API account, mainnet send, provisioned VPS or
funded operation was used in these checks. The independent recovery CLI is read-
only unless explicitly passed `--send`, constructs one known instruction and
has fee/rent/program-identity checks. These controls do not remove operator-price
trust or establish independent source-state freshness.

The indexer/API/web preserve a distinct defaulted status, nominal q loss, lender
collateral, actual keeper reward and account refund. Due rent is not reported as
paid rent; no buyback cost, token return or trading PnL is invented. Shared-ledger
idempotence/rebuild and legacy repayment/API compatibility have focused PostgreSQL
checks. Final retained receipts distinguish earlier intermediate binaries from
the final locked-vesting checkpoint.

**Remaining release work is material:** no public quote-native adoption/opening
path is exposed, no production general Jupiter financial executor is enabled,
and shared-pool deposit/withdraw/sweep, typed margin/close/liquidation, actual
snapshot debt hooks and per-cohort carry handling still need complete integration.
The final fuzzer must generate those real paths and qualify frozen final inputs
for at least 24h; catalog/standalone tests are not that evidence. Actual source/API
capacity, configured public authorities, funded operator/alerts/backups and the
concrete multisig deployment packet remain pending. This is an engineering
checkpoint, not a mainnet-ready source tag or deployment approval.


The [retained October28 checkpoint](probes/trusted-history-2026-10-28/README.md)
contains exact current bytes, scope-specific counts, resolved review findings,
independent AI review receipts and lossless logs. Current release raw hash is
`43971efd45e22df7e2511e5439ff78281f4fa72c30b88bfc96e7f82740008ac1`;
this development candidate is not a final release/fuzz freeze. Public mainnet
role identities and the prospective sub-lamport cohort rule were requested from
the owner while implementation continues; neither is inferred from placeholders.
