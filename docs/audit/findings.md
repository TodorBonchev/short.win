# short.win audit findings

This file tracks every finding from the external audit and records how each was resolved. It
belongs to the audit package ([`docs/AUDIT.md`](../AUDIT.md) §12). The M10 exit criterion is
"every finding fixed, or acknowledged with a rationale" (ARCH §9).

**Rules:**
- **One row per finding**, keyed by the firm's id. Keep the firm's severity, and add our own
  only if we disagree; give the reason in the finding's section below.
- **Fixed** means all of the following:
  - the fix commit is on the audit branch;
  - a regression test fails on the audited commit and passes on the fix (a LiteSVM test, a
    fuzz corpus file under `tests/fuzz/corpus/`, or a `shortwin-math` test);
  - the full gate (G-full) is green on the fix commit;
  - a ≥ 24 h fuzz run on the fix commit has 0 failures;
  - the auditors' fix review has accepted it.

  Program fixes go out in batches, so that hashes, CU baselines and fuzz runs are redone once
  per batch.

  *2026-10-13:* the owner waived the ≥ 24 h fuzz run for the AI review's fix batch (AIR-01 …
  AIR-24, [`ai-review/README.md`](ai-review/README.md)); no such run was made on its program.
  The rule above is unchanged for the audit's findings.
- **Acknowledged** means not fixed, with a written rationale and the owner's sign-off. The
  finding then gets a row in [`known-issues.md`](known-issues.md), with its bound.
- **Duplicate of KI-n** means the finding restates a known issue within its stated bound.
  Answer it by pointing to the row. If the finding exceeds the bound, it is a new finding.
- **A fix that changes a bound** in `known-issues.md` or the design documents updates them in
  the same commit.

**2026-10-21 current disposition:** the owner confirmed the program has never been
deployed and delegated the recommended policy selections. The
[owner decision record](owner-decisions-1.0.3-2026-10-21.md) acknowledges the exact
fee, cashback, keeper-payment, pot-priority, carry and deposit-availability residuals.
The current batch uses the owner-authorized [focused AI fix review](independent-ai-fix-review-1.0.3-2026-10-21.md)
and [integration review addendum](independent-integration-review-1.0.3-2026-10-21.md),
with disclosed provenance and separate peer review in place of an external firm report.
Source `e04a6776` includes the subsequent TLS, replay, legacy keeper and live-venue
integration corrections; [full CI 39507546399](https://github.com/TodorBonchev/short.win/actions/runs/39507546399)
and the [new final-profile ≥24-hour run](fuzz-1.0.3-fullsha.md) remain required.
The live venue byte identities and bounded compatibility evidence are in the
[preflight record](live-preflight-1.0.3-2026-10-21.md). Historical sections below
retain their original review state; this dated disposition supersedes their pending
owner/external-review prerequisites for the selected scope.

## Tracker

Status is one of: open, in progress, fixed, acknowledged, duplicate of KI-n, invalid.

| ID | Title | Firm severity | Our severity | Location | Status | Fix commit | Regression test | Fix review | Notes |
|---|---|---|---|---|---|---|---|---|---|
| AIR2-01 | Locked lender's rounding share after rebase | — | not assessed | `pool::convert_steps` | in progress | this batch | Direct converted lock/zero-lock control; isolated old-floor negative control | [Independent AI review accepted implementation](independent-air2-fix-review-1.0.3-2026-10-21.md); CI green; final-profile fuzz pending | Nonzero lock rounded up; review repaired a regression masked by L39. The original all-free-inventory magnitude remains unverified. |
| AIR2-02 | Stale EMA accepted at open | — | not assessed | `open_short` | in progress | this batch | `ai2_stale_ema_it` | [Independent AI review accepted implementation](independent-air2-fix-review-1.0.3-2026-10-21.md); CI green; final-profile fuzz pending | Recent integration and completed observation required; includes a preceding crank and 40-second boundary. |
| AIR2-03 | PumpSwap start sample contaminates warm-up | — | not assessed | `prologue::run` | in progress | this batch | `ai2_pumpswap_start_it` | [Independent AI review accepted implementation](independent-air2-fix-review-1.0.3-2026-10-21.md); CI green; final-profile fuzz pending | Skip recording the initial price as a warm-up bucket; KI-18/KI-22 do not cover this single-transaction start. |
| AIR2-04 | Late deposit captures halted-S3 vest | — | not assessed | `sol_settle`, `deposit`, `pool::settle` | in progress | this batch | `ai2_halted_s3_vest_it`; pool hold unit test | [Independent AI review accepted implementation](independent-air2-fix-review-1.0.3-2026-10-21.md); CI green; final-profile fuzz pending | Related to AIR-07/AIR-09 and KI-71; extends beyond the drained-pool gate. |
| AIR2-05 | Dust flush leaves unswept cashback exposed | — | not assessed | `pool::settle`, `sweep` | in progress | this batch | `ai2_accounting`; `ai2_venue_review_it`; keeper dust-flush scenario | [Independent AI review accepted implementation](independent-air2-fix-review-1.0.3-2026-10-21.md); CI green; final-profile fuzz pending | Dust flush gates deposits with surviving shares; pending-source income is credited immediately or held through split-claim vest release. Legacy source bits are rechecked, with keeper/web/indexer integration. |
| AIR2-06 | New governance key need not sign | — | — | `apply_config` | duplicate of KI-70 | — | existing AIR-04 evidence | existing owner decision | Same unsigned-authority behavior accepted on 2026-10-12; no new bound asserted. |
| AIR2-07 | Chosen-pair collision in truncated ELF fingerprint | — | not assessed | `flag_venue_upgraded` | in progress | `f5a8ccef` | Full-32 suffix matrix, signed flag/revalidation/extension, 304-byte layout and client roundtrip | Independent AI fix review accepted implementation; CI green; final-profile fuzz pending | Full SHA-256 replaces the 12-byte fingerprint before first deployment; no old-layout migration needed. [Owner choice](owner-decisions-1.0.3-2026-10-21.md); [AI review](independent-ai-fix-review-1.0.3-2026-10-21.md). |
| AIR2-08 | Settlement arm survives venue restart | — | not assessed | `settle::evaluate` | in progress | this batch | `settle::tests::air2_08_*`; I18; legacy marker regression | [Independent AI review accepted implementation](independent-air2-fix-review-1.0.3-2026-10-21.md); CI green; final-profile fuzz pending | Persistent restart count+1 invalidates pre-restart arms even with no warm-up call; legacy arms invalidated once at first prologue. |
| AIR2-09 | Basis-point rounding lowers locked depth after LP deposit | — | not assessed | `pool_select::locked_depth` | in progress | this batch | raw-ratio unit/property tests | [Independent AI review accepted implementation](independent-air2-fix-review-1.0.3-2026-10-21.md); CI green; final-profile fuzz pending | Raw LP fraction replaces whole-bps depth; normative model and fixtures synchronized. Real AMM v4 PnL recognition remains separate. |
| AIR2-10 | Setting coin creator raises risk fee | — | not assessed | `pump_amm::read_state` | acknowledged | — | needed | Owner-delegated rationale, 2026-10-21 | Retain current creator/live fees and capped risk policy. Owner-delegated acknowledgement on 2026-10-21 accepts fee-change risk decisions without a demonstrated worst-case magnitude; [decision record](owner-decisions-1.0.3-2026-10-21.md). |
| AIR2-11 | Risk-fee cap creates an exit-budget gap | — | not assessed | `sol_settle`, `bounds` | acknowledged | — | needed | Owner-delegated rationale, 2026-10-21 | Retain risk/live quote distinction. Owner-delegated acknowledgement accepts possible exit-budget divergence with no universal exit deadline or loss cap established; [rationale](owner-decisions-1.0.3-2026-10-21.md). |
| AIR2-12 | Cashback paid into a noncanonical trader WSOL account | — | not assessed | `sweep_trader` | acknowledged | — | `ai2_venue_review_it` characterization | Owner-delegated rationale, 2026-10-21 | Canonical authentication retained. Outsiders can strand unbooked cashback, potentially all repeatedly diverted rewards; 545,713 lamports is a fixture sample, not a cap. Recovery/completeness are not implemented. [Owner-delegated acknowledgement](owner-decisions-1.0.3-2026-10-21.md). |
| AIR2-13 | Keeper receives zero after rent-first liquidation | — | not assessed | `waterfall::liquidate` | acknowledged | — | `ai2_accounting`: rent exhaustion | Owner-delegated rationale, 2026-10-21 | Rent-first priority retained; keeper payment can be zero despite a nominal floor. Existing keeper attempts eligible exits using its funded payer; a funded/monitored operator is a deployment prerequisite, not a verified service. [Owner decision](owner-decisions-1.0.3-2026-10-21.md). |
| AIR2-P2-01 | One-token top-up bypasses a fully locked allowance | — | not assessed | `pool::allowance` (L39) | in progress | this batch | `ai2_accounting`: L39 control comparison | [Independent AI review accepted implementation](independent-air2-fix-review-1.0.3-2026-10-21.md); CI green; final-profile fuzz pending | Unlocked-value cap independently reviewed. Old formula replay withdraws 670,224 (net 670,223) versus 336,779 control; new candidate refuses it. KI-13 remains separate. |
| AIR2-P2-02 | Pot pays a first claimant against unpaid receivable | — | not assessed | `pool::claim` | acknowledged | — | `ai2_accounting`: receivable/pot allocation | Owner-delegated rationale, 2026-10-21 | First-claim shared-pot priority and current-share shortfall policy retained. Unequal realised payouts and later uncollectible claims are accepted if covers never arrive; no small absolute/lifetime cap. [Owner-delegated acknowledgement](owner-decisions-1.0.3-2026-10-21.md). |
| AI3-FR01 | Already-empty legacy pool retains carry across upgrade | — | not assessed | `pool::deposit_in` | in progress | `e1d66caa` | Canonical predecessor witness, carry matrix, signed legacy deposit; old source/ELF negative controls fail | Narrow quarantine independently reviewed; ≥24 h fuzz/release requirements pending | New deposits refuse 6110 when S=0 and either carry is nonzero, preserving historical value/claims. Full exact-ref CI passed: 25 jobs, 19 mutants killed, verifier/Anchor ELFs byte-identical. On 2026-10-19 the owner confirmed no affected deployed state: current-deployment recovery is not applicable. The owner confirmed never-deployed status on 2026-10-21. The earlier separate-machine 20 h checkpoint is historical; [current final-profile fuzz](fuzz-1.0.3-fullsha.md) must complete ≥24 h with zero failures/anomalies before **fixed**. [Fix, status, hashes and CI](ai-review-3/ai3-fr01-fix-1.0.3.md). |

## AI review 3: draft 1.0.3 (2026-10-18)

The [immutable audit archive](ai-review-3/README.md) covers `b815269`, the preceding
draft 1.0.2 source. Two passes produced eight findings and 23 leads; all findings have
confidence 75, **not a severity rating**. One first-pass specialist failed and second-pass
coverage was narrower. The [31-item triage](ai-review-3/triage.md) gives every item a
decision and prior-issue relationship. This follow-up does not certify that every lead is
exploitable or that the remaining program is safe.

| ID | Draft action / disposition | Status | Evidence |
|---|---|---|---|
| AI3-F01 | Use `min(P_ema, P_candidate)` for the keeper window floor, matching the program | in progress | Candidate-price regressions in `pool_monitor` |
| AI3-F02 | Continue searching after a candidate-specific floor failure, including an existing window chain | in progress | Deeper failing / shallower passing candidate regressions |
| AI3-F03 | Resolve PumpSwap coin-creator placeholder 4 from the cached creator | in progress | Current/previous extras and stable cached-key regression in `venue` |
| AI3-F04 | Multiply the raw locked LP fraction before rounding SOL depth | in progress | Sub-basis-point positive-depth and current-supply regression |
| AI3-F05 | Refresh the pool LP counter with the vault and LP mint, in complete groups of at most 99 accounts | in progress | Pool-34 boundary and production keeper ranking scenarios |
| AI3-F06 | Assign whole-lamport entitlement and realised carries to the final departing lender, normalize surplus and clear both carries | in progress | `pool_carry` conservation and real-transaction `lender_it` regressions |
| AI3-F07 | Existing unsigned successor-key behavior | duplicate of KI-70 | AIR-04 / AIR2-06 and owner decision of 2026-10-12; no new bound |
| AI3-F08 | Existing hashless ProgramData extension latch | duplicate of KI-69(a) | AIR-01 residual; retain immediate hashed bootstrap and revalidation procedure |
| AI3-L01 | Reject an ALT shorter than its complete metadata header | in progress | `tx::tests::lookup_tables_require_the_complete_metadata_header` fails before / passes after |
| AI3-L03 | Reject unsupported versions at the keeper's typed account readers | in progress | Typed-reader regressions in `view` |
| AI3-L04 | Preserve all pending rank candidates until every RPC group succeeds | in progress | First/second-group failure and successful retry without rediscovery; [internal handoff](handoff-1.0.3-2026-10-20.md) adds sustained outage and short-response coverage while cached candidates continue |

These implemented rows remain **in progress** until the current-profile ≥24-hour
zero-failure fuzz and remaining release requirements pass. [Final full CI 39509811932](https://github.com/TodorBonchev/short.win/actions/runs/39509811932)
passed on exact source `ff85a1b0`: 25 successful jobs, zero failures, one intentional
scheduled-fuzz skip and all 19 selected mutants killed. Verifier, Anchor and frozen
program bytes match. The earlier `9b91060` CI/fuzz evidence describes preceding bytes.
The [current readiness record](release-readiness-1.0.3-2026-10-21.md) identifies the
304-byte/full-digest guard and current frozen fuzzer/fixtures; no tag or deployment is
approved by this record. The earlier AIR-01 … AIR-24 fuzz waiver does not cover this batch.

All 20 unimplemented leads retain explicit dispositions. Current-writer/key constraints
bound the hardening hypotheses in L07/L09/L11/L14/L17. Unverified production triggers,
configuration maxima and aggregate L23 strategies are not silently declared safe or
fixed. The owner-selected actual-vest hold, fee/risk distinction, canonical cashback,
rent-first keeper payment, pot entitlement and final-holder carry retain the exact
acknowledged consequences in the [decision record](owner-decisions-1.0.3-2026-10-21.md).
Full SHA-256 is implemented before first deployment; no old-account migration is needed.
Technical implementation review and economic acknowledgement remain separate.

### Independent fix review and unresolved treatment (2026-10-19)

**Historical review snapshot:** this subsection preserves the 2026-10-19 review state.
The 2026-10-21 owner decisions and attributed review reports linked above supersede
its then-pending owner/review choices; the current-profile fuzz gate remains open.

The [independent review](ai-review-3/independent-fix-review-1.0.3.md) identifies the
candidate and verified CI artifacts, source comparisons, newly executed checks and each
packet answer. Q2 accepts F01–F05, Q3 accepts L03/L04 and Q4 accepts L01. Q1 accepts
F06's post-fix final-withdrawal behavior only conditionally for an upgrade: an already-empty
pre-fix pool is not migrated (AI3-FR01). This is independent AI review work, not an
external firm's sign-off or a full-system audit. Implemented rows remain **in progress**.

The [written treatment](ai-review-3/unresolved-1.0.3.md) and
[known-issues §8](known-issues.md#8-open-items-from-the-103-fix-review) cover the six
prior open findings, all 20 unimplemented review-3 leads, and the carry allocation/migration
boundaries. They give the evidence limits, rationale and completion requirements. None is
newly **acknowledged**, and no older KI acceptance is broadened. Release still requires
resolution or explicit owner sign-off on the exact residuals, the final-program ≥24 h fuzz
evidence and the remaining integrated-revision gates.

### Carry ownership at the final withdrawal

**2026-10-19 follow-up:** [AI3-FR01's deposit quarantine](ai-review-3/ai3-fr01-fix-1.0.3.md)
now prevents an already-empty legacy pool from admitting a new cohort while either carry
is nonzero. It preserves the historical balances and old allocated claims. The owner
confirmed on 2026-10-19 that no affected deployed state exists, so current-deployment
recovery/migration is not applicable; reviewed recovery would still be required before
reopening any affected legacy pool encountered later. The earlier review's finding
remains evidence of the pre-guard behavior. Full CI on `e1d66caa` passed, including
mutations and the verified release rebuild. The owner reports 20 hours with zero
anomalies in a separate-machine 24-hour fuzz run; final-byte provenance and completed
≥24 h zero-failure evidence remain pending in the linked fix record.

Both accumulator carries hold value not yet assigned to individual lenders. At high raw
token/share supplies they can contain whole lamports; retaining them after `S = 0`
lets a later cohort receive the preceding cohort's income. The draft first flushes the
vest and touches the final lender, assigns `carry / WAD` to pending and `carry_r / WAD`
to realised SOL, then normalizes any realised surplus into the existing unmatched pot.
It clears both carries and adds only their sub-lamport remainders to diagnostic dust.
Existing zero-share lenders keep their allocated pending claims and backing. This follows
the existing final-holder vest rule; it does not reconstruct historical pro-rata ownership
of unallocated carry among lenders who left earlier. Partial withdrawals,
rebase records and the prohibition on removing all shares while tokens remain borrowed
retain their existing behavior. This is not a redesign of the historical pot policy.
The [internal review](handoff-1.0.3-2026-10-20.md) reproduced the residual with
signed transactions: a lender who enters with one raw token after income has
vested can receive 1,000 lamports of the earlier holder's carry on the final
exit. Its bounded last-holder policy is unresolved; this row remains in progress.

The regression against the previous release ELF completes two cohorts with signed
deposits/withdrawals and actual System donations plus permissionless trader sweeps. It
fails because the departing lender receives 1,000,000 instead of 1,001,000 lamports;
the next cohort receives the missing 1,000. Unit negative controls also fail for carry
mismatches and sub-lamport residue handling. Implementation checks and final-code gate
evidence are separated in the [1.0.3 validation record](ai-review-3/validation-1.0.3.md).

## AI review 2 working-tree batch (2026-10-18)

The batch is now committed as a draft 1.0.2 candidate. The heading and local validation
record retain their pre-commit provenance. In the tracker, `this batch` identifies the commit
that adds `ai-review-2/triage-pass2-2026-10-18.md`; no release or auditor acceptance is implied.

These are AI review identifiers, not firm findings or firm severity ratings. Confidence scores
in the original report are not severity. The immutable source is
[`ai-review-2/report.md`](ai-review-2/report.md); the comparison register is
[`known-issues.md`](known-issues.md), including its AIR-01 … AIR-24 mapping. The broader targeted triage covers all 85 pass-1 leads and maps all 330 pass-2 blocks
to 74 exact keys. Evidence is in [math/accounting](ai-review-2/triage-math-2026-10-18.md),
[venues/program](ai-review-2/triage-venues-2026-10-18.md),
[services/governance](ai-review-2/triage-services-2026-10-18.md) and the
[pass-2 register](ai-review-2/triage-pass2-2026-10-18.md). This is not the missing original
judge run, pass 3, an external fix review or owner acceptance of any open issue.

AIR2-02 … AIR2-04 already had uncommitted implementations at the start of this follow-up. Their
regressions now run in `tests/litesvm/tests/`. AIR2-02 checks the pre-prologue `ema_ts` and the
completed bucket's `lb_sec`, so an old bucket rolled by a preceding crank is still stale.
AIR2-03 keeps the start observation out of the subsequent warm-up buckets. AIR2-04 stores
`deposit_hold_until` in eight reserved Market bytes (account size unchanged); deposits wait
for the actual shared vest to release, even when a later credit extends it. The broader review
reproduced a split-settlement bypass of the half-claims threshold; every non-flushing settlement
leaving shares and a live vest now holds deposits.

The generated IDL/client and the web's open/deposit gates follow these changes. The hold reuses
`PoolDrained` and changes deposit availability: a sequence of credits can prolong it. The field
is a hold marker, not an authoritative unlock timestamp: -1 means initialized/no hold, zero
means legacy/uninitialized, and a positive value records a hold. First accrual on a legacy
market invalidates both source bits and arms the hold, because old dust-flushed accounts can
have no vest but still have unclaimed cashback. A successful deposit resets the marker to -1
after source cleanup and actual vest release. Pending required rewards also arm the hold:
an empty Trader sweep followed by a later cashback claim can re-vest that income through the
Trader path, so completed source bits alone cannot reopen deposits. Both new PumpSwap dust
regressions fail on the preceding draft and pass on the corrected binary; the late depositor
receives zero old cashback. The S3 regression covers legacy live-vest migration and later vest
extension. A second reserved
u64 records observation restart identity; Market remains 3,104 bytes. The source package is
now **1.0.2**, a draft candidate with no release tag or deployment.

When cleanup is pending with surviving lender shares, the web now offers a separately
reviewed claims-and-sweep transaction. Bundling a deposit could start a new vest and then
roll the whole cleanup back at the deposit guard. The separate action keeps the entered
deposit intact, sends no deposit instruction, and re-reads on retry; deposits wait until
cleanup completes and any actual vest releases. Empty pools retain the atomic sweep/deposit
flow. The review explains the network fee and any cashback account rent.

The short UI now offers a wallet-reviewed refresh for a positionless idle market after its
normal listing and venue prerequisites pass: record a price observation, wait for a later
confirmed chain second, then separately review/sign a crank to integrate it. Reloading resumes
from confirmed market state. Missing Clock data and an integration review whose observation
has expired hold the action; a refresh never automatically opens a position. The keeper does
not acquire a continuous paid cadence for positionless markets. The interaction and fee
disclosure are documented in [`../design/FRONTEND.md`](../design/FRONTEND.md) §3.7.
The real local-validator refresh test passed twice, including reload recovery and checking
that the second transaction completed the first transaction's observation without opening a
position. The earlier draft's web validation passed 1,050 unit tests (1 skipped), typecheck and lint. The
Android MWA suite passed 19 tests (2 flag-off skips) on a fresh ledger with the draft program.

Historical verification of the earlier draft before the broader fixes (2026-10-18): the math/WASM suites passed 387 tests;
the LiteSVM suite passed 1,142 tests with 5 ignored (the complete run plus reruns of the
fixtures updated for the new observation rules). The fixture changes add real observations
and preserve the existing economic assertions. Strict root- and services-workspace Clippy
checks passed. All 20 release fuzz smoke tests passed after adding fresh observations to the
historical coverage fixtures, retaining their original seeds, amounts and assertions.
The PumpSwap and halted-S3 regressions were also run against CI's verified original 1.0.1:
both fail at their economic assertions; the latter pays the late depositor 5,865,111,095
lamports of the old lender's vest. The same regressions pass on this draft.
Canonical `anchor build --tools-version v1.57 --arch v3 --ignore-keys` and
`scripts/build-test-venue.sh` produced these file SHA-256 hashes:

| Local draft artifact | SHA-256 |
|---|---|
| `target/deploy/shortwin.so` | `a367935057f900444ea2bdefe981f9935441382c1248a73c9262bb37f3a9bfdc` |
| `target/test-venue/deploy/shortwin.so` | `c3689cc688690ffdaa247a25e16d652a73d504e887a2d97df95d65ef4f791897` |

These earlier 1.0.1-versioned test artifacts are superseded by the broader 1.0.2 candidate.
They are not verifiable-build evidence or hashes of the current working tree.

### Draft 1.0.2 validation (historical)

After the broader fixes and the legacy/split-claim source-accounting follow-up, the canonical
builds produced these local artifacts. AUDIT §0 records their sizes and executable hashes.

| Historical local artifact | SHA-256 |
|---|---|
| `target/deploy/shortwin.so` | `a6ecd6f763f9549eee5913a08d57effbcd69dd312d59217a44cc386f03e62b5e` |
| `target/test-venue/deploy/shortwin.so` | `7449c9917d63652a7d36b9cded09a8c26137f38ce9c1513dfb7ed0d19f1505e7` |

Completed checks on the current candidate:

- The full program/core/LiteSVM run passes **1,343 tests**, with **5 ignored**:
  1,147 LiteSVM tests and 196 program, venue-reader, WASM and Squads tests. The separate
  math suite passes **376 tests**. The ignored cases remain outside this evidence.
- All **21 release fuzz smoke tests** pass after integrating the upstream regression below.
  Both organic PumpSwap source-accounting
  regressions fail at the deposit guard on the preceding draft and pass on this one.
- After the standalone cleanup UI follow-up, the web passes **1,065 tests**, with **1 skipped**,
  plus typecheck and lint. Its focused cleanup/component/review checks pass **79 tests**.
  The regenerated 120-case lender corpus also passes its focused preview suite; no coverage threshold was
  reduced. The pure-math recorder explicitly uses a venue without required rewards.
- On the final SBF artifacts, **12 selected local-validator browser cases** pass: idle-price
  refresh, post-wipe deposit, both claim-sync flows and phone lending. After the standalone
  cleanup UI change, the **two-case claim-chain scenario passes again on a fresh ledger**.
  Its reviewed cleanup lands exactly one `SweepTrader`, preserves shares/epoch and completes
  the source check. The subsequent claim pays all 348,205 credited lamports with zero pending,
  leaving the absent lender unconverted. These are selected browser regressions, not the
  entire repository browser gate.
- M1/M2 and M3/M5/M6 CU/trace/depth measurements are refreshed against these bytes;
  the published budgets remain unchanged. All **12 transaction-size probes** pass.
- M7 keeper cost, cascade and PumpSwap transaction-size gates pass on these bytes.
  The measured inputs, simulator output and ECON §5.8 tables are refreshed together;
  nine PumpSwap observations now cover price start and warm-up. The largest measured
  market-day cost increase is **0.00104 SOL** under the benchmark's assumptions.
- The alternate no-LTO/16-codegen-unit SBF build has no stack-frame error after isolating
  the adoption calculation. All **40 pool-selection tests** pass against that artifact
  and the canonical build. The alternate build is diagnostic, not a release artifact.
- The normative models pass **16 focused tests**, **180,000 pool operations**, **4,800
  Monte Carlo paths**, **300 exit-fuzz runs** and the **1,655-case configuration battery**.
  All 20 pool JSON fixtures reproduce byte-for-byte.
- Strict root-workspace/program Clippy and formatting pass. The final release artifact
  passes A49 (no mock venue), SBPF v3 and `security.txt` checks for `v1.0.2`.
- The regenerated JavaScript client passes typecheck and **29 tests**, including Market's
  unchanged account size and all 107 field offsets.
- Services workspace and standalone keeper Clippy and formatting pass. Both offline ZIP 9
  extraction regressions pass, covering generic ZIP, Zonky wrappers, CP437 filenames and
  truncated archives; the dependency patch adds no advisory exception.
- The corrected full services workspace run passes **409 tests**, with **6 ignored**,
  including PostgreSQL migrations, API/security ingestion, indexer replay, keeper scenarios
  and governance execution. Two ignored benchmark cases are covered by the separate M7 run;
  four live-RPC cases remain outside this suite's evidence.

Reproduction details and measurement files are linked from the three triage reports.
Local logs include `/tmp/shortwin-ai2-broader-all-core-final.log`,
`/tmp/shortwin-ai2-fuzz-smoke-post-source-hold.log`,
`/tmp/shortwin-ai2-alt-sbf-pool-select-final.log`,
`/tmp/shortwin-services-full-final-corrected.log`, `/tmp/shortwin-ai2-web-after-cleanup.log`,
`/tmp/shortwin-ai2-web-e2e-final.log` and `/tmp/shortwin-ai2-web-cleanup-chain-final.log`.
These checks establish local regression
evidence; they do not complete the original audit passes or external fix review.

Before committing/pushing, this batch was rebased onto `9062dab`, preserving the upstream
wallet/link UI changes and FUZZ-101-24H-A oracle fix. The web again passes 1,065 tests plus
typecheck/lint. The new fuzz smoke needs fresh observations before its first idle open and
a trader sweep after its dust flush to reach the historical rebase under 1.0.2. Only its
test setup adds those prerequisites; its original 14-action corpus, oracle and economic
assertions remain intact. All 21 fuzz smokes, formatting and strict fuzz Clippy pass on
the integrated tree. Logs: `/tmp/shortwin-audit-push-web.log` and
`/tmp/shortwin-fuzz101-final-{smoke,fmt,clippy}.log`. Integration changes no program, math,
client or services source from the validated candidate, so the SBF artifact hashes above
remain unchanged.

**Still required for release:** resolution or explicit acceptance of the remaining findings,
the full release gate on the committed source revision, verifiable hashes
for those exact bytes, the ≥24 h fuzz run and auditor fix review under AUDIT §12. The version
and legacy vest/arm migration mechanics are implemented; that is not a completed release. The successful
CI verifiable build on `35558c2` is for the earlier 1.0.1 bytes only.

## Pass-1 lead inventory and broader triage

IDs follow the source report’s order. Every lead has an evidence row in the linked review;
open means unresolved, not proven valid or accepted. Pass 2’s blocks are fully inventoried,
with targeted reproductions; its missing original judge, pass 3 and an independent full
keeper-core audit remain outstanding. The services review includes selected keeper-core
call paths and regressions, not just the two binaries in the original scan.
Implemented or locally fixed below describes draft changes, not the release-level
**fixed** status defined at the top of this tracker.

| ID | Lead | Broader disposition and evidence |
|---|---|---|
| L01 | The extend command never adds a market's keys to that market's table. | Confirmed; draft implementation tested. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L02 | The v1 probe prints `landed` after a refused v1 transaction. | Confirmed; implemented in the draft batch. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L03 | A pool switch keeps the market's route-broken flag set. | Current behavior; policy question, no automatic clearing added. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L04 | check_fee_recipient accepts System-owned PDAs of short.win as the recipient. | Governance-input risk, standard tool constrained. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L05 | check_shape accepts any feed id in an OracleFeed change. | Partly hardened; semantic choice remains governance's responsibility. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L06 | apply_config installs an OracleFeed account it never reads. | Operational failure confirmed; mitigated in stage/apply. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L07 | Any caller can fund the boost vault ATA and block PumpSwap opens | Open, bounded availability trigger. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L08 | The execution fit reads the candidate's live real SOL and spot price. | Conditional liveness lead remains open. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L09 | The fee-scaled caps count the creator fee as a manipulation cost. | Open economic assumption. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L10 | carried_caps takes the execution tier from the candidate's live SOL reserve | Duplicate mechanism of L08; remains open for nondefault caps. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L11 | check_change validates the stored parameters for a non-parameter change. | Future-upgrade obligation. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L12 | check_change revalidates stored parameters for a key or oracle change | Same future-upgrade obligation, tracked separately. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L13 | check_fee_recipient accepts a System-owned short.win PDA | Duplicate of L04; operational constraint documented. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L14 | check_trade skips key checks when a venue account does not parse | Conditional check, no current bypass established. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L15 | claim_sol runs the lender touch at the raw clock, not market_now | Standalone future-check allegation unproven; related clock mismatch hardened. [math evidence](ai-review-2/triage-math-2026-10-18.md). |
| L16 | claim_sol moves the lock calendar past the market's last accrual time | Conditional correctness bug reproduced; fixed locally. [math evidence](ai-review-2/triage-math-2026-10-18.md). |
| L17 | classify_pump_with reads a pump-owned curve with another discriminator as NotPump | Open future-layout behavior. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L18 | classify_pump_with returns NotPump without a read of the canonical pool | Open detection assumption. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L19 | close_short reports every trader sweep as SWEEP_TO_VEST. | Confirmed event inconsistency; draft correction. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L20 | close_short accepts a position in any status. | Future status hardening, no current alternate live status. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L21 | crank_price re-detection seeds P_ema with no open warm-up. | Open re-detection policy. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L22 | crank_sol_usd refuses large moves while tiers still use the old price. | Existing oracle policy, not a new freshness bypass. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L23 | The Raydium listing ignores the pump latch bits | Dependency-scope policy remains open. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L24 | Latch readers load VenueGuard without its version check | Hardening implemented. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L25 | The first create_market caller picks the venue, pool and market age | Permissionless first listing is intentional; adversarial pool-choice bound open. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L26 | curve_pricing takes the PumpSwap fee tier at a fixed 10¹⁵ supply | Known formula distinction, not a new fixed-supply implementation bug. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L27 | Risk quotes past rT take a fixed seed of 206.9e12 tokens | Historical versus current synthetic state must be distinguished. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L28 | deposit checks the absolute vault balance, not the received amount | No current underpayment path established. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L29 | Pool selection adopts a pool on a venue the Emergency flagged | Specified flag scope; no code change. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L30 | A 60-second observation gap cancels an armed collection window | Covered continuity rule, not a demonstrated bypass. [math evidence](ai-review-2/triage-math-2026-10-18.md). |
| L31 | exit_risk_cap_bps caps the risk fee at P_ema on PumpSwap only | Open comparison of distinct quote policies. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L32 | fee_view skips the consistent() check of read_state | Open fee-only read hardening. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L33 | Venue latch fails open after a loader-v4 migration | Future-loader support gap retained. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L34 | graduate stores the PumpSwap vault keys and never reads the latch | Expected exit scope. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L35 | pool_vault has no token::mint or token::authority constraint in the exits | Construction invariant, future hardening. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L36 | liquidate, expire, sol_settle and close_short never read position.status | Future-state hardening obligation. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L37 | ShortPosition carries no layout version that the exits check | Future-layout obligation. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L38 | lp_supply accepts a 165-byte token account as an LP mint | Decoder hardening; no reachable account substitution found. [math evidence](ai-review-2/triage-math-2026-10-18.md). |
| L39 | The allowance value cap counts a lender's locked shares | New pass-2 bound violation confirmed; fixed locally. Not covered by KI-13. [math evidence](ai-review-2/triage-math-2026-10-18.md). |
| L40 | check_sim allows a minimum rent above the cr_open to cr_liq gap | Dangerous in-bounds configuration confirmed; open configuration-policy choice. [math evidence](ai-review-2/triage-math-2026-10-18.md). |
| L41 | A keeper earns more from a later liquidation than from expire. | Reward asymmetry confirmed; strategic delay/lender harm unresolved. [math evidence](ai-review-2/triage-math-2026-10-18.md). |
| L42 | Each venue read copies every pump-fees tier onto the heap | Open compute/resource bound. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L43 | A one-unit position stops the last lender's vest flush | Covered accepted vest-forfeiture behavior. [math evidence](ai-review-2/triage-math-2026-10-18.md). |
| L44 | Raydium exits still reserve the pump ert of a carried position | Within documented carried-term behavior. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L45 | A burn before open_short raises the fee that sizes the caps. | Open opening-capacity bound. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L46 | open_short skips the ema_warm check on curve opens. | Open curve warm-up policy. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L47 | The open_short deny list omits the trader's token accounts. | Open extras hardening; no current transfer path. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L48 | open_short moves the sell refund with only a rent floor. | Open refund attribution review. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L49 | open_short prices the post-open state on the entry mint supply. | Open pre/post supply consistency. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L50 | open_short reads the VenueGuard latch without a version check. | Duplicate of L24; hardening implemented for opens. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L51 | open_short computes the D4 margin at the live PumpSwap fee. | Open margin/fee comparison. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L52 | open_short takes no bound on the locked minimum rent. | Execution-time exposure confirmed; open terms/API choice. [math evidence](ai-review-2/triage-math-2026-10-18.md). |
| L53 | open_short takes no bound on the tier it assigns. | API policy. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L54 | open_short applies the real-SOL floor on Raydium only. | Venue-specific policy. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L55 | open_static checks only the curve bit on a curve open. | Open dependency-mask policy. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L56 | The extras deny lists omit trader_base_ata | Same unresolved extras question as L47. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L57 | parse_extras rejects a read-only extra that the transaction passes writable | Confirmed strict composition behavior. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L58 | A PumpSwap config that does not parse blocks every live curve exit. | Venue dependency availability. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L59 | post_complete_known accepts every curve whose ivqr is zero. | Open post-complete invariant hardening. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L60 | Pool selection ignores the candidate's venue_enabled and venue_route_broken flags | Duplicate of L29; policy retained. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L61 | VenueGuard readers skip the VENUE_GUARD_VERSION check | Duplicate version-check gap; hardened in both pool-selection readers. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L62 | propose_pool lists a Raydium pool while the other Raydium program awaits review | Confirmed, explicitly pinned scope distinction. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L63 | A pool switch keeps graduated_at from a thin old pool | Retained-age policy. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L64 | pump_cashback_claimable reads a UVA with a foreign owner as empty | Future-owner hardening gap. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L65 | Raydium parse reads the vaults with no check of their token authority. | Future-authority hardening gap. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L66 | Graduation drops the ρ factor that the migrating FV applies to P_ema. | Known transition residual. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L67 | revalidate_venue accepts only the exact slot for an ELF above 2 MiB. | Known size/availability residual, retained. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L68 | revalidate_venue records the expected hash unchecked at a matching slot. | Authority trust boundary, not independent hash verification. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L69 | A PumpMigrating market without pricing settles S3 at ρ = 1. | Open fallback reachability/bound. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L70 | The sol_settle no-op stores MarketVault only when three fields changed | No current mutation omitted; future API obligation. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L71 | sol_settle passes market.token_program to mint_guard as the token program | No external token-program substitution established. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L72 | A same-transaction buy lowers the PumpSwap fee in sol_settle's fair value | Open, grouped with AIR2-11. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L73 | sol_settle times a position's S3 wait from the market's lower-cap chain | Documented low-water chain. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L74 | An AMM v4 deposit moves PnL into need_take_pnl and lowers locked_depth | Distinct unresolved real-venue effect, not fixed by AIR2-09. [math evidence](ai-review-2/triage-math-2026-10-18.md). |
| L75 | store_records writes 16 bytes into accounts that it does not check | Internal API hardening lead, no current unchecked caller. [services evidence](ai-review-2/triage-services-2026-10-18.md). |
| L76 | A donation blocks deposits if claim_cashback ever needs the trader's signature | Future upstream-interface dependency. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L77 | sync_after_leg passes CanonicalPool::Absent and never reads C3. | Open lifecycle check. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L78 | sync never graduates a migrating market whose curve no longer parses | Open upgrade/layout availability. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L79 | Any caller's sync_lender advances the lock calendar at the raw clock | Same confirmed conditional bug as L16; fixed locally. [math evidence](ai-review-2/triage-math-2026-10-18.md). |
| L80 | sync sets no graduated_at when a PumpMigrating market moves to None | Open transition age semantics. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L81 | The Raydium readers treat a pool with a frozen vault as executable | Open executable-classification hardening. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L82 | parse leaves the coin_creator placeholder unresolved on curve markets. | Confirmed unsupported template placeholder. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L83 | token_account::read accepts a 355-byte Token-2022 multisig as a token account. | Reader hardening implemented. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L84 | swap_enabled reads an AMM v4 pool at status 5 as halted. | Open pinned-ELF status behavior. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
| L85 | Only Anchor's duplicate-account check refuses pool_vault as the destination | Current alias refusal already enforced. [venues/program evidence](ai-review-2/triage-venues-2026-10-18.md). |
