# Draft 1.0.3 readiness (2026-10-20)

Source candidate: **`b8183266655def0195ad18608f103d15ebc03ac6`**.
This integrates `audit-1.0.3-gates` at `49293595` with `main` at `297c8f65`,
including AI3-FR01's empty-pool deposit quarantine. The merge is `eae77d9a`;
keeper evaluation recovery is `16d661d5`, and web history recovery is `b8183266`.
No tag or deployment has been made. **The release gates below remain open.**

## Engineering work completed

The handoff's ranking-retry correction was independently reviewed. A separate
candidate-evaluation RPC failure still returned before maintaining the current pool's
failure observations. Its new sustained-outage regression fails on the preceding code
at a 31-second observation gap. Failed and incomplete sibling reads now preserve the
observation cadence only when this tick's independently read incumbent actually fails
and has zero positions. They leave evaluation due and return before proposing from
stale candidate data. Fresh sibling reads restore normal proposals. Healthy and
unreadable incumbents do not become new failure evidence.

The Liquidations tab could stay empty after a missed websocket event or an invalidation
that hit the API's short cache before ingestion. Mounted visible exit feeds now refresh
every 10 seconds. Position history refreshes every 10 seconds while incomplete or open,
and every 60 seconds once terminal. This also recovers a missed rollback message.
The position page shows history updating while the chain account has closed but the
indexer still reports open; recorded terminal history suppresses cached pre-exit controls.
Polling stops while hidden or unmounted.

Both signed carry regressions survive the integration: the pre-entry final-holder
ownership characterization and AI3-FR01's quarantined legacy state with preserved old
claims. The original AI3-L23 two-release floor trace was compared with accepted KI-68
and recorded as a duplicate within that per-touch bound. Aggregate/design extensions
remain open in the [decision packet](release-decisions-1.0.3-2026-10-20.md).

| Local check | Result |
|---|---|
| Keeper pool-selection scenarios | 18 passed, including sustained ranking/evaluation outages, incomplete responses, healthy/unreadable incumbent controls and recovery |
| Keeper and keeper-LiteSVM strict Clippy | All targets passed with `-D warnings`; services formatting passed |
| Math carry regressions | 10 passed |
| Signed lender adversarial tests | 20 passed on the integrated release ELF |
| Fuzzer self-tests and smoke/corpus suite | 12 harness tests and 21 smoke/corpus cases passed; not a 24-hour result |
| Final whole-web unit suite | 1,073 passed, one intentional skip, across 90 passing files |
| Focused web history/live regressions | 16 passed, including missed websocket/rollback recovery and hidden/unmounted controls |
| Web typecheck and changed-file Biome | Passed |
| Root Rust formatting | Passed |

## Integrated CI and final-byte fuzz

The combined candidate requires an exact-ref full `ci.yml` dispatch, including
mutations, dependency checks, keeper scenarios, browser E2E and the verified build.
The previous green runs on `9b91060` and `e1d66caa` remain revision-specific evidence.
The completed integrated result and downloaded artifact comparison will be recorded
here when available.

The [new all-profile fuzz run](fuzz-1.0.3-fr01.md) started on 2026-10-20 against frozen
program and runner binaries from this source candidate. Its ≥24-hour completion is
pending. Release/test-venue program hashes reproduce the prior
[FR01 artifact record](ci-1.0.3-fr01-hashes.txt); the old
[completed 24-hour run](fuzz-1.0.3.md) used different program bytes and is historical.
Any subsequent program/fuzzer change requires a new final-byte run.

## Decisions and review before release

The [six-choice decision packet](release-decisions-1.0.3-2026-10-20.md) provides concrete
options and evidence for VenueGuard digest length, risk/execution fees, alternate-account
cashback, keeper payment, historical pot allocation and final-holder carry ownership.
None is newly accepted by this engineering pass. Remaining AI2/AI3 leads retain their
explicit dispositions and evidence limits; measuring them is not equivalent to accepting
an unspecified residual.

The owner previously confirmed no affected deployed AI3-FR01 state. Recovery/migration
of that state remains not applicable to the current deployment. This confirmation
does not accept other historical fund-allocation issues.

The external auditors' fix review required by [AUDIT §12](../AUDIT.md#12-logistics-and-change-policy)
is still required for the AIR2/AI3 batches and keeper follow-up. Internal independent
review and the local checks above supply reviewer input, not that external acceptance.

After the gates and exact owner decisions, update formal finding statuses and the final
release hash table. Deployment order still requires an indexer supporting sweep
destination `3` before the program, followed by compatible keeper/client/web versions.
The owner must review the tag and deployment before those actions.
