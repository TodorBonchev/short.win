# Draft 1.0.3 audit handoff (2026-10-20)

**Historical handoff:** the integration with `main` now also includes AI3-FR01's
quarantine and newer program bytes. The completed fuzz run described here remains
evidence for `9b91060` only. Current policy choices are collected in the
[release decision packet](release-decisions-1.0.3-2026-10-20.md); the work below
records the original handoff, before the evaluation-read and frontend follow-ups.

The working branch is `audit-1.0.3-gates`. **Do not tag or deploy 1.0.3 yet.**
The integrated program/fuzzer candidate is `9b910602ed076a6d4359c496960b3ffc0e7c6af8`.
Its [exact-ref CI run](https://github.com/TodorBonchev/short.win/actions/runs/39144291500)
passed 25 jobs, and the [24-hour all-profile fuzz run](fuzz-1.0.3.md) completed
with zero fuzzer failures and anomalies on the same release ELF. This handoff
adds keeper source and characterization tests; it does not change program or
fuzzer source. A direct source diff against `9b91060` is empty for the program,
math implementation and fuzzer, and the local release/test-venue/fuzzer hashes
still match the fuzz record. The old CI run does **not** cover the new keeper code. A new
exact-ref full CI run is required on the handoff commit. If a later change
modifies program or fuzzer bytes, repeat the 24-hour fuzz gate as well.

## Work in this handoff

A fresh internal code review found that a ranking RPC failure returned from
`select_market` before evaluating its already ranked candidates. Repeated
failures could block a usable sibling pool and interrupt the zero-position
`pool_fail` observation chain. The change in
[`keeper.rs`](../../services/keeper/src/keeper.rs) keeps the rank queue
uncommitted on transport errors or short responses, reports the error, then
continues with cached candidates. The
[LiteSVM scenarios](../../services/keeper-litesvm/tests/scenarios/pool_select.rs)
cover first- and later-group errors, short responses, pool 34 at the 100-key
boundary, and more than 60 failing ticks while a cached sibling is proposed
at the first armed second. Focused scenarios, scoped Clippy, formatting and
`git diff --check` passed.

The review also reproduced a **separate, unresolved ownership residual** in
AI3-F06. A deposits 2,000,000,000,000,000,000 raw tokens. A 1,001,000-lamport
sweep fully vests, allocating 1,000,000 lamports to A and leaving 1,000
lamports in accumulator carry. B then deposits one raw token. If A exits
before B, A receives the 1,000,000 lamports and B receives the earlier 1,000
lamports on the final exit. The
[math characterization](../../crates/shortwin-math/tests/pool_carry.rs) and
[signed LiteSVM transactions](../../tests/litesvm/tests/lender_it/adversarial.rs)
both pass. This is consistent with the implemented final-holder rule, not
proof that historical ownership is correct. The tested 1,000 lamports alone
do not show a profitable attack after fees. At the pool's ratio cap, one
carry is bounded by about 18,446,744 lamports (0.01845 SOL) if market token
claims cannot exceed the mint's `u64` supply; a fresh pool's initial share
ratio bounds it to 18,446 lamports. There is no aggregate bound across
repeated cycles. Do not classify AI3-F06 as an economically complete fix
without an owner decision on this residual.

The separate source reviewers found no new scoped implementation defect in
AI3-L01 or AIR2-01–04, AIR2-08/09 and AIR2-P2-01. AIR2-05's dust-flush gate
works for rewards reaching the canonical accounts, but its completeness
depends on AIR2-12: PumpSwap can permissionlessly send cashback to another
trader-owned WSOL account that the canonical sweep cannot recover. The
[existing pinned-ELF test](../../tests/litesvm/tests/ai2_venue_review_it.rs)
demonstrates it. These are **internal AI-assisted source reviews**, not an
external auditor's fix acceptance. The [fix-review packet](ai-review-3/fix-review-1.0.3.md)
remains open.

Focused verification in this pass: `ai2_accounting` 16/16; the AIR2
LiteSVM targets 10/10; the restart-marker unit test 1/1; `pool_carry` 8/8;
the new signed carry transaction 1/1; the focused keeper retry and sustained
outage scenarios passed. The earlier full CI/fuzz evidence is linked above.
No new full CI or 24-hour fuzz run was executed after the keeper change.

## Written technical dispositions; owner decisions still required

These are conservative engineering recommendations. They are **not owner
risk acceptance** and do not change any row in [findings.md](findings.md)
from `open` or `in progress` to `acknowledged` or `fixed`.

| Finding | Decision for this handoff | Remaining release condition |
|---|---|---|
| AIR2-07 / AI3-L12 | Do not apply a layout-only patch. Keep the 12-byte VenueGuard fingerprint open. A fixed ELF needs roughly 2^96 work to match; an authority choosing both ELFs has a roughly 2^48 chosen-pair search. No executable collision is known. | Versioned full-digest migration, or an owner-signed acknowledgement of the chosen-pair risk and its venue-authority trust assumption. |
| AIR2-10/11 and AI3 fee/supply leads | Keep the risk-fee cap and live execution quote until both sides are tested together. A stranger can change the current PumpSwap coin creator on the pinned ELF; a harmful healthy-position exit and a persistent exit-budget gap have not been executed. | Pinned transactions for creator and fee-tier changes, both settlement box corners and live budget; then a joint economic design decision or bounded owner acknowledgement. |
| AIR2-12 | Treat alternate WSOL cashback as confirmed and unresolved; a canonical-only sweep can mark rewards clear while value remains outside it. Do not remove the ATA check by itself. | Account discovery/authentication and historical claim-completeness design, or explicit owner acceptance of stranded cashback and later-cohort allocation risk. |
| AIR2-13 | Keep rent-first accounting unchanged pending policy. A tested 20 SOL vault, 19 SOL buyback and 1 SOL rent pays the keeper zero despite a 0.21 SOL nominal floor. | Owner chooses a reserve/priority change or signs that keeper payout and service are not guaranteed in this case. |
| AIR2-P2-02 | Treat first-claim pot allocation as confirmed; current-share subtraction is not a safe correction. The test pays 75/25 SOL and leaves a late lender at −64 SOL pending. | Historical entitlement or new cash-policy design with regression, or explicit owner acceptance of an advantage bounded by the unmatched pot, with no small absolute bound established. |
| AI3-F06 carry residual | Retain the last-exit implementation only as a draft technical fix; late entry can take pre-entry carry as reproduced above. | Owner chooses bounded final-holder rounding or commissions an ownership redesign and signed transaction regression. |
| AI3-L10 | Do not bypass the vest-based deposit hold with a timestamp alone; that could restore AIR2-04 dilution. Repeated small donations may prolong the hold, but the reported cycle lacks a full production transaction. | Production reproduction and owner liveness decision, including an unbounded-duration disclosure if accepted. |
| AI3-L23 | The exact one-lamport per-touch trace appears within owner-accepted KI-68; it can be recorded as a duplicate only within that per-touch bound. | Check and record the comparison; investigate any aggregate strategy beyond KI-68 separately. |

The other still-open AI3 leads (L13, L16, L19–L22) and substantive AI2
economic/configuration leads remain in their [triage registers](ai-review-3/triage.md)
and [findings tracker](findings.md). No unsupported future-state hardening lead
was promoted into a current exploit by this pass.

## Next steps for the next maintainer

1. Review the keeper diff and consider the separate evaluation-read error at
   `services/keeper/src/keeper.rs` near `select_market`'s candidate
   `self.chain.accounts(&keys).await?`: it can still return before
   `keep_pool_fail_chain`. A focused injected-read regression should prove the
   intended chain behavior before changing it.
2. Run the full local services/program gate and an exact-ref
   [`ci.yml`](../../.github/workflows/ci.yml) dispatch on the committed branch;
   confirm the new keeper tests, all 19 mutants, verifiable ELF equality and
   browser jobs. The old 24-hour fuzz evidence remains tied to the unchanged
   program/fuzzer bytes; if a later fix changes them, repeat the run.
3. Obtain a real external fix review covering the AIR2 and AI3 batches and
   the new keeper correction. The internal pass above is useful reviewer
   input, not the auditors' sign-off required by
   [AUDIT §12](../AUDIT.md#12-logistics-and-change-policy).
4. Resolve the confirmed fund-allocation issues or obtain the owner's
   explicit written rationale and bounds for each accepted risk. Update
   `known-issues.md` and `findings.md` only after that decision.
5. After those gates, update the exact candidate/hash table, decide the
   tag and deployment order (indexer before program for the new sweep
   destination), and have the owner review the release. No release action
   has been taken here.
