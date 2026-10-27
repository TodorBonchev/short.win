# short.win 1.0.3 mainnet launch plan

**Superseded launch scope (owner, later 2026-10-27): first mainnet is postponed
for broad Jupiter redesign.** See the [new decision](../audit/owner-jupiter-redesign-decision-2026-10-27.md)
and [design proposal](../design/JUPITER_REDESIGN.md). The plan below preserves the
1.0.3 baseline and operating requirements; it is not authorization to tag/deploy
that candidate while the redesign is unresolved. Audit/bounty timing remains
post-launch as previously selected.

Current owner decision: **launch mainnet with existing per-market borrowing caps
before an external security audit and before a funded bug bounty**. Use protocol
fee revenue to accumulate funding for those later security engagements. The owner
confirmed this on 2026-10-27 (Europe/Kyiv), including the explicit choice to leave
deposits and market count uncapped. The
[decision and risk acceptance](../audit/owner-mainnet-decision-1.0.3-2026-10-27.md)
supersedes earlier audit-first sequencing and delegated no-go timing.

This document is the current release plan. Older dated audit/devnet records retain
what happened at the time. This policy change does not report an external audit,
close unvalidated findings, fund a bounty, demonstrate keeper availability or make
the mainnet deployment live. The existing website/devnet deployment is separate.

## What is capped

The unchanged program enforces `market_max_notional`: the sum of the opening
notionals of currently open shorts in each market. Its compiled default is
**25 SOL per market** (`25_000_000_000` lamports). Other existing price-impact,
fee-scaled size, utilization and wallet-share limits also apply. Verify the actual
initialized on-chain configuration; do not rely on the example file alone.

**Deposited tokens, SOL margin, marked-to-market exposure, the number of markets
and aggregate protocol TVL/exposure are not capped by that parameter.** Market
creation is permissionless. The keeper's market list and the website's listings
restrict those services only. An emergency opens pause does not block deposits,
withdrawals, claims or permissionless market creation. There is no maximum
protocol-wide loss guarantee. Avoid describing this launch as globally TVL-capped.

Selected public wording:

> Unaudited mainnet launch with per-market borrowing limits. The configured
> opening-notional limit is 25 SOL per market. Deposits and the number of markets
> are uncapped. No funded bug bounty is currently available. Participation can
> result in loss of funds.

This is a release-policy change, not a change to program economics, account
layouts, fee shares or the canonical runtime. Lowering a limit later requires a
reviewed initialization/configuration packet and any applicable timelocks; the
current governance example initializes compiled defaults.

## Audit and security funding

External audit, contest and funded bounty are post-launch work rather than
prerequisites for this initial release. Earlier AI-assisted reviews, their exact
fix scope, peer-review limits and qualified fuzz evidence remain accurately
attributed. The 2026-10-22 whole-repository checkpoint remains incomplete: its
seven hypotheses needing validation, 23 pending independent dispositions, eight
blocked source units and two stopped tasks are not cleared by this decision.
The owner accepts proceeding with that incomplete review. Do not retry the
provider-stopped tasks through this launch-policy work.

Maintain the existing private vulnerability reporting channel immediately under
[SECURITY.md](../../SECURITY.md); responsible disclosure is available without a
reward promise. Obtain external audit and bounty quotes, record their funding
targets, and publish a funded program only when its reserve, scope, rules and
payment process actually exist. Revenue and dates are uncertain.

Only earned **protocol fees** swept to the configured treasury/fee recipient may
fund this plan. Keep a separate security reserve ledger after necessary operating
and zero-reward-exit reserves. Do not divert lender deposits, allocated lender
income, position margin or deficit-cover obligations. This decision changes no
on-chain fee allocation and performs no treasury payment. Report reserve balance,
quotes and funding progress; do not describe future receipts as secured funding.

## Release qualification and tag

**Current live blocker (2026-10-27):** the aggregate read-only fixture check failed
at slot 461,877,333 because Token-2022 redeployed at slot **460,497,271**, replacing
the tested slot 427,147,035. Independent finalized inspection at slot 461,878,038
confirmed different executable bytes (611,056 bytes; SHA-256
`3dde54d202caa2d10514fd58a154df5f26f07643f9f5fd0402ff08084f0cb954`).
This is a real code change, not a header-only redeploy. Pump/PumpSwap/pump-fees,
Raydium, Jupiter, Squads and memo matched the dated check. Retained
[observations and exact diagnostic bytes](../audit/probes/mainnet-preparation-2026-10-27/README.md)
do not promote a new fixture or qualify the old fuzz against that new upstream ELF.
The [new scoped source/provenance review](../audit/probes/mainnet-preparation-2026-10-27/token-2022-source-review/REPORT.md)
now attributes both complete ELF files to the publisher's actual release assets,
and retains an external public-verifier match for the new source commit. Static
comparison identifies no required adapter change within the existing mint
allowlist. That is not a new-ELF execution test or a fresh local reproducible build.
Complete the remaining Token-2022 initialization/ATA/memo/CPI execution tests,
applicable fork tests and qualifying evidence before the final mainnet-ready tag.
Stable `v1.0.3` remains pending that technical gate; audit-first timing is not the
reason for this hold. The [new baseline qualification record](../audit/token2022-qualification-1.0.3-2026-10-27.md)
retains the successful slot-461,913,288 live check, unchanged release bytes, partial
CI results and the browser clock repair; the repaired full gate and fresh campaign
remain pending. The program ID itself had no mainnet account at finalized
slot 461,879,034, as recorded in the same evidence.

Base for this preparation: main `7f2af5b5787d62978c79e7630f5b0e1a3310d94e`.

[Full CI 40434130363](https://github.com/TodorBonchev/short.win/actions/runs/40434130363)
passed on `57bc1c845`: 25 jobs passed, zero failed, one intentional nightly skip,
19 selected mutants killed, both keeper startup regressions executed, and Anchor/
verifier release bytes matched. [Retained complete result and hashes](../audit/probes/mainnet-preparation-2026-10-27/ci-40434130363/README.md).
That run uses the old Token-2022 fixture; draft qualification work adds the new
baseline separately and cannot borrow the earlier fuzz duration.
The canonical release ELF SHA-256 remains
`d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc`
(1,644,024 bytes), trimmed executable hash
`f3d8dd7a5c62385abc6306e2da395ff666a1f69234ea50f2e7eb0523ab686250`.
The [byte-rebuild record](../audit/probes/release-wrap-2026-10-22/byte-rebuild/SUMMARY.md)
and [completed frozen fuzz](../audit/probes/fuzz-final-1.0.3-2026-10-22/README.md)
establish the original evidence and its equivalence assumptions. The completed
run exceeded 24 hours with zero invariant failures and zero anomalies; it does
not give each profile 24 hours or test later keeper/web changes.

1. Finish the policy/documentation change and any current CI repair. Preserve
   raw review archives, checksums and old failed CI/live results.
2. Run scope, diff/whitespace and static fixture checks; repeat the read-only live
   fixture/dependency check and retain its timestamp, slots and output hashes.
   Investigate new upstream drift before promoting any fixture. The old Jupiter
   drift was already dispositioned; that dated pass is not today's observation.
3. Require green CI on the selected revision and relevant service/web checks.
   Verify unchanged program/fuzzer/fixture inputs and actual release bytes against
   the qualified evidence. Any relevant runtime change needs new qualification.
4. Integrate the prepared revision into main and create annotated **`v1.0.3`**
   identifying the final code/artifact and this unaudited launch policy. The owner
   has requested this final tag; another generic audit-first sign-off is not
   required. A tag identifies code and is not evidence that mainnet operations
   have been commissioned or permission to claim deployment happened.
5. Record the selected commit, CI links, artifact hashes and tag object. Keep
   `v1.0.3-devnet.1` as a historical prerelease; never move an existing tag.

## Commission the mainnet operator before enabling opens

Follow [MAINNET_OPERATIONS.md](../deployment/MAINNET_OPERATIONS.md) for the VPS,
RPC, service commands, funding, alerts, backups and retained public evidence.
These technical requirements remain even though audit/bounty timing changed:

- Choose a production VPS and paid mainnet RPC with WebSocket/history capacity.
  Keep the indexer database and metrics private, expose the API behind TLS, and
  monitor from outside the VPS. The workstation/tunnel devnet stack is not the
  production availability plan.
- Install version/hash-pinned keeper, indexer and client artifacts. Verify the
  304-byte/full-digest guard and indexer sweep destination `3` compatibility.
- Provision separate fee-payer and coverage hot keys. Keep deployment/governance
  signing keys off the VPS. Supply real Governance/Emergency members, thresholds,
  timelocks, derived vaults, fee recipient and exact launch configuration; the
  checked governance example contains unsendable placeholders.
- Restore the normal active oracle and fast keeper profile with `dry_run=false`.
  The devnet oracle-disabled, 300-second paused/empty profile and bounded free-RPC
  relay must not be copied to mainnet. Cover all markets admitted for new shorts,
  and monitor permissionless market creation beyond the initial serviced set.
- Fund the operator before admitting users. Demonstrate active-position coverage,
  threshold/burst handling, a zero-reward exit, RPC recovery, backup restoration,
  manual payer top-up and an independently received alert/opens-pause drill.
  Observation budget is not an exit-spending cap. Reserve for rent, creation,
  graduation, oracle and loss-making keeper transactions separately.

No VPS account, funded mainnet payer, reachable independent alert recipient or
mainnet governance member set was supplied in this request. Those remain concrete
operator inputs, not claims completed by repository tests.

## Deployment sequence

1. Produce a read-only mainnet packet: correct genesis, actual program address and
   current loader/account state, reviewed release ELF, deployment payer and balance,
   upload-buffer/upgrade authority, mainnet venue slots and full fingerprints,
   oracle account/feed, governance identities and fee/rent cost envelope. Verify
   whether this is genuinely a first mainnet deployment. Devnet state is not proof
   of mainnet state.
2. Review and simulate the exact loader/configuration transactions and bounded
   costs. Obtain approval of that concrete financial/signing packet before any
   submission. This request specifies policy, tag and planning; it supplies no
   mainnet transaction amounts or signing identities.
3. Deploy only the canonical release build, initialize/check governance and
   configuration, verify the program bytes and public account identities, and
   complete all required venue revalidations before permitting opens. Establish
   an opens pause during commissioning; verify that it is actually effective.
4. Start the compatible indexer before publishing mainnet readers. Start the funded
   active keeper and coverage pauser, confirm landed observations and independently
   delivered alerts, then perform a small owner-funded open/close and exit drill.
   Independently verify transactions and balances; no synthetic test is a receipt.
5. Enable new opens only after those operator checks pass. Publish the actual
   program address, tag/commit, limits, unaudited status, current bounty status,
   governance powers, covered-market policy and residual risks on the mainnet site.
   Keep devnet and mainnet configurations visibly distinct.
6. Watch closely through the first 24 hours, pause new opens on lost coverage,
   RPC/oracle/venue drift or an unexplained accounting discrepancy, and retain
   exit service/funding. Review burn and treasury receipts daily; do not raise
   limits or advertise a funded bounty merely because fee income begins.

An external audit and bounty remain priority funded follow-up work. Publish
findings and remediation as they occur; continue to disclose the accepted
cashback, fee/risk, keeper-payment, pot-priority, carry and deposit-denial risks
in the [known-issues register](../audit/known-issues.md).
