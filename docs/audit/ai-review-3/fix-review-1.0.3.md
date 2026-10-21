# Draft 1.0.3 fix-review packet

**Final disposition (2026-10-22):** the packet's technical scopes F01–F06,
L01/L03/L04 and the follow-up AI3-FR01 quarantine are now **fixed** on the
recorded regressions, accurately attributed AI review, green exact-ref
[CI 39509811932](https://github.com/TodorBonchev/short.win/actions/runs/39509811932)
and [completed final-profile ≥24-hour fuzz](../probes/fuzz-final-1.0.3-2026-10-22/README.md).
The separate AIR2-07/L12 full-digest correction is also fixed. Q1's historical
deployed-state condition is satisfied by the owner's never-deployed confirmation,
not by an independently asserted chain inventory or an implemented migration.

The [final release checks](../release-checks-1.0.3-2026-10-22.md) and
[findings tracker](../findings.md#final-technical-disposition-2026-10-22) give the
source, coverage limits and remaining operating conditions. AI3-F06's final-holder
ownership residual and the owner-selected economic/availability risks remain
acknowledged. Deferred leads and scoped duplicates retain their bounds.
This closes technical fix review under the batch-specific authorized AI route;
it is not external-firm acceptance or tag/deployment approval.

## Historical request and intermediate dispositions (2026-10-18–21)

The questions, old candidate identities and pending gate statements below are retained
as their original dated request/evidence. The final disposition above supersedes those
status statements without changing their historical measurements.


**Current disposition (2026-10-21):** this packet's original review request and
pre-decision boundaries are historical. The [owner-delegated decisions](../owner-decisions-1.0.3-2026-10-21.md)
confirm no deployed old state and select the documented economic policies. Complete
SHA-256 is implemented before first deployment. Accurately attributed
[focused](../independent-ai-fix-review-1.0.3-2026-10-21.md),
[integration](../independent-integration-review-1.0.3-2026-10-21.md) and
[AIR2](../independent-air2-fix-review-1.0.3-2026-10-21.md) AI reviews provide this batch's
owner-authorized review route, with implementation overlap disclosed; no external
firm's acceptance is supplied or claimed. The [current readiness record](../release-readiness-1.0.3-2026-10-21.md)
tracks final exact-ref CI and current-profile ≥24-hour fuzz. Neither the review route
nor never-deployed status waives those execution gates.

**Follow-up candidate:** [AI3-FR01's guard and regression](ai3-fr01-fix-1.0.3.md)
refuse new deposits into legacy empty pools retaining carry. The hashes and CI below
identify the preceding candidate; they do not cover the guard's new bytes. The owner
confirmed on 2026-10-19 that no affected deployed state exists, so current-deployment
recovery/migration is not applicable. The linked fix record contains that confirmation
and the ongoing fuzz status; the original review request below remains historical.

This packet is for an independent review of the 1.0.3 candidate after the audit of
`b815269224ba0f5d946f49de7e113220c4067041`. It is a review request, not a
review or a sign-off. The [original report](report.md), [31-item triage](triage.md)
and [validation record](validation-1.0.3.md) are separate: the report describes the
old tree; the latter two describe the draft changes and their limits. The exact
integrated candidate is `9b910602ed076a6d4359c496960b3ffc0e7c6af8` on
`audit-1.0.3-gates`. Its [CI run 39144291500](https://github.com/TodorBonchev/short.win/actions/runs/39144291500)
is attempt 1 of a `workflow_dispatch` on that SHA. The release and test-venue file
hashes, tool image and verifier/Anchor byte comparison are in the
[CI hash record](../ci-1.0.3-hashes.txt). The run passed 25 jobs with only the scheduled
fuzz job skipped. The separate [24-hour all-profile fuzz run](../fuzz-1.0.3.md)
completed cleanly on those preceding program bytes. It does not cover the newer
AI3-FR01 guard; the final-byte ≥24 h gate remains open. The
[independent AI fix review](independent-fix-review-1.0.3.md) of 2026-10-19 accepts
Q2/Q3/Q4 and conditionally accepts Q1. The owner subsequently confirmed no affected
deployed state, so current-deployment recovery is not applicable; see the
[FR01 fix record](ai3-fr01-fix-1.0.3.md). The later
[internal handoff](../handoff-1.0.3-2026-10-20.md) adds a keeper retry correction;
its full CI and the external auditor's fix acceptance remain required. The original
questions below remain the review request, with the answers and boundaries in the
linked review and [written treatment](unresolved-1.0.3.md).

## Questions for the reviewer

1. **Final-share carry (AI3-F06).** Check that `pool::withdraw_in` assigns the
   whole-lamport entitlement and realised carries to the last lender in the same
   order as the on-chain withdrawal handler, normalizes any realised surplus into
   the pot, and leaves earlier zero-share lenders' pending claims intact. Review
   signed rent true-up, mismatched carries, partial burns, dust, overflow rollback,
   and the first deposit of a new cohort. The code deliberately follows the
   existing final-holder vest rule; assess the remaining historical allocation
   question rather than treating this fix as a complete redesign of pot ownership.
2. **Keeper candidate choice (AI3-F01–F05).** Check the candidate-specific
   `min(EMA, candidate spot)` floor and the search after a deeper candidate fails,
   including an already armed failure chain. Check PumpSwap creator placeholder 4
   in all extras sets, raw-fraction LP ranking, and the refreshed three-account
   pool snapshot with RPC groups no larger than 99 keys.
3. **Keeper recovery and readers (AI3-L03/L04).** Check that an RPC error retains
   the whole ranked batch for retry, and that every typed GlobalConfig, Market,
   VenueExtras and VenueGuard read rejects unsupported versions without making a
   valid account unreadable.
4. **ALT metadata (AI3-L01).** Check that a partial metadata header cannot be
   accepted as an empty lookup table and that complete empty tables still parse.

The carry [negative control and transaction tests](validation-1.0.3.md#regression-evidence)
use the preceding release ELF, actual signed lender transactions and a new-cohort
draw. Keeper negative controls reintroduce each old behavior in isolated copies.
The [combined checks](validation-1.0.3.md#combined-checks) include builds, model
and fixture reproduction, service tests, fuzz smoke and compute budgets. Archive
integrity can be checked with `sha256sum --check SHA256SUMS` in this directory.

## Boundaries requiring a separate decision

This candidate does not resolve the prior [open findings](../findings.md#tracker):
full-digest VenueGuard migration (AIR2-07), creator and risk-fee treatment
(AIR2-10/11), noncanonical cashback recovery (AIR2-12), rent-exhausted keeper
incentives (AIR2-13), or historical pot entitlement (AIR2-P2-02). The
[triage](triage.md) also retains the donation hold, tier and repeated-accrual
questions. Prior KI-69 and KI-70 decisions are retained only within their
documented bounds. Review of this fix batch must not imply acceptance of these
separate issues.

The independent AI fix review is **recorded, with Q1 conditional**. Its new finding
AI3-FR01 remains open; external firm acceptance is not supplied by this work. The
findings tracker may mark a fix **fixed** only after its own commit, negative and
positive regression, full gate, zero-failure 24-hour fuzz run, and required fix-review
acceptance. None of the separate unresolved findings is newly accepted by this report.
