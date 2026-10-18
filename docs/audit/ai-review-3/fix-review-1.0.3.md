# Draft 1.0.3 fix-review packet

This packet is for an independent review of the 1.0.3 candidate after the audit of
`b815269224ba0f5d946f49de7e113220c4067041`. It is a review request, not a
review or a sign-off. The [original report](report.md), [31-item triage](triage.md)
and [validation record](validation-1.0.3.md) are separate: the report describes the
old tree; the latter two describe the draft changes and their limits. The exact
candidate commit and CI run must be recorded before sign-off.

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

Independent fix review is **pending**. A reviewer should identify the candidate
commit and release/test-venue ELF hashes, record any findings and their resolution,
and state explicitly whether the four questions above are accepted. The findings
tracker may mark a fix **fixed** only after its own commit, negative and positive
regression, full gate, 24-hour fuzz run, and this independent review.
