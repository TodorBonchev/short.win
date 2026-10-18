# AI review 3: draft 1.0.2, two passes

This folder archives the AI-assisted static review completed on 2026-10-18 against
`b815269224ba0f5d946f49de7e113220c4067041` (draft 1.0.2), using
[rust-auditor](https://github.com/TodorBonchev/rust-auditor) at
`6c41e1d46386de7fd9b1fa48c847e876ac8b2676`. It is not a formal audit. The scan used
Codex with the model inherited from its runtime; the provenance does not record a
specific model identifier.

The original result is in [`report.md`](report.md). Current implementation work for
draft 1.0.3 belongs in [`../findings.md`](../findings.md); the
[`31-item triage index`](triage.md) and [draft validation record](validation-1.0.3.md)
track the follow-up. The [fix-review packet](fix-review-1.0.3.md) states what an
independent reviewer needs to assess; the [2026-10-19 independent AI fix review](independent-fix-review-1.0.3.md)
records Q2/Q3/Q4 acceptance, Q1's legacy-state migration condition (AI3-FR01), and
the [written unresolved treatment](unresolved-1.0.3.md). These follow-up documents
are separate from the immutable original scan. The index assigns stable `AI3-F01`–`AI3-F08` and
`AI3-L01`–`AI3-L23` IDs without changing the historical evidence.

## Status and limits

| Item | Recorded result |
|---|---|
| Passes | Two requested and completed: pass 1 returned 11 of 12 specialist reports; pass 2 returned all 12. Pass 1's economic-security specialist failed before its final report and was not retried. |
| Candidates | Pass 1: 36 raw candidates, reduced to 8 findings and 22 leads. Pass 2: 47 raw candidates, reduced to 8 findings and 14 leads. All 83 raw candidates have dispositions. |
| Combined result | **8 findings, all at confidence 75, and 23 unscored leads. Confidence is not severity.** Deduplication across passes yields 31 items. |
| Prior records | 21 exact-known mechanisms, 7 related but distinct extensions, and 3 new to the inspected project register. The report's `NEW` means absent from the initially empty local ledger; it does not establish project novelty. |
| Scope | 119 Rust files selected from the core and keeper classes of `scripts/audit_scope.py`; test/mock code excluded. Governance and other keeper code could be consulted as context, which does not extend the declared production scope. |
| Reading coverage | The combined full-production-file union is 119 of 119. This means at least one completed specialist recorded a full production read of each file, with narrower reads under individual lenses. It does not mean every specialist read every file. |
| Validation | Static source review and arithmetic reasoning. No `--poc`, builds, runtime tests or executable reproductions ran during this audit. |
| Patch checks | Pass 1's 10 alternatives and pass 2's 11 selected finding options passed individual `git apply --check`. Of pass 2's 19 distinct raw patch texts, 18 passed. All were unapplied and uncompiled at the time of the scan. |

Pass 1's file union was 119; the failed economic-security review remains a missing lens
despite that union. Pass 2 recorded 88 full production reads, 8 bounded-only files,
17 search-only files and 6 files whose records leave unread versus search-only ambiguous.
The latter 31 files are reading limits, not unfinished specialist reports. The final
[`pass-1 coverage`](pass-1/coverage.json) and [`pass-2 coverage`](pass-2/coverage.json)
retain the per-file and per-specialist evidence.

The proposed patches also have implementation limits. Refreshing an LP counter alone
does not make separate RPC snapshots coherent. Moving empty-pool carries into dust does
not pay former lenders or reclassify the SOL backing. Requiring venue hashes needs a
bootstrap path for existing hashless deployments. The successor-signature proposal needs
IDL and client integration. The raw four-site position-status hardening patch failed its
applicability check and is retained as evidence. These proposals are not release-ready
merely because another option applies; follow-up implementation and its tests are recorded
separately.

## Main artifacts

| File | Contents |
|---|---|
| [`report.md`](report.md) | The pinned upstream assembler's `full-report.md`, copied byte for byte under this archive name |
| [`run-1.md`](run-1.md), [`run-2.md`](run-2.md) | Both judged pass reports, unchanged |
| [`triage.md`](triage.md) | The 31 stable IDs, canonical keys, prior relationships and implementation-status table |
| [`fix-review-1.0.3.md`](fix-review-1.0.3.md) | Draft fix-review questions and unresolved boundaries, linked to the independent AI review; external firm acceptance is not supplied |
| [`independent-fix-review-1.0.3.md`](independent-fix-review-1.0.3.md) | Independent AI fix review: candidate/artifact identity, Q1 conditional, Q2/Q3/Q4 accepted, and new AI3-FR01 |
| [`unresolved-1.0.3.md`](unresolved-1.0.3.md) | Written treatment of the six prior open findings, all 20 unimplemented review-3 leads and carry boundaries; no new owner acceptance |
| [`account-map.md`](account-map.md) | Instruction accounts, constraints, helper locations and generated review leads |
| [`provenance.json`](provenance.json), [`final-verification.json`](final-verification.json) | Pinned commits, procedure, final counts, limitations and the original scan's consistency checks |
| [`files.txt`](files.txt), [`scope.tsv`](scope.tsv) | Frozen scope and run scope metadata |
| [`source-sha256.json`](source-sha256.json), [`source-names.tsv`](source-names.tsv) | The 119 source hashes and source index, sufficient to identify the reviewed revision without copying source bundles |
| [`existing-issues.json`](existing-issues.json), [`prejudgment-context.txt`](prejudgment-context.txt) | Prior project records and context supplied before judgment |
| [`project-issue-comparison.json`](project-issue-comparison.json) | All 31 canonical keys mapped to raw candidates, exact prior sources and relationship reasons |
| [`coverage-check.json`](coverage-check.json) | Final pass-1 coverage reconciliation, also preserved in `pass-1/coverage.json` |
| [`coverage-check.txt`](coverage-check.txt), [`coverage-progress.json`](coverage-progress.json) | Historical intermediate coverage snapshots; use the final per-pass JSON for the final coverage result |
| [`SHA256SUMS`](SHA256SUMS) | SHA-256 hashes of all 90 unchanged archived artifacts; excludes follow-up indexes, triage, validation, fix-review and unresolved-treatment documents |

## Candidate judgments and patch evidence

Each candidate has a stable raw ID such as `p1-a01-b01`. Its `artifact` names the
specialist report below. Each pass's `decisions.json` preserves one disposition per raw
ID, the canonical grouping, execution/reachability/trigger/impact gates and reasons.
The prior comparisons describe historical relationships independently of those verdicts.

| Evidence | Pass 1 | Pass 2 |
|---|---|---|
| Raw candidate blocks | [`candidates.json`](pass-1/candidates.json) | [`candidates.json`](pass-2/candidates.json) |
| Every disposition and final item | [`decisions.json`](pass-1/decisions.json) | [`decisions.json`](pass-2/decisions.json) |
| Comparison with prior project records | [`prior-comparison.json`](pass-1/prior-comparison.json) | [`prior-comparison.json`](pass-2/prior-comparison.json) |
| Accounting/admin judgment inputs | [`gate-input-accounting_admin.json`](pass-1/gate-input-accounting_admin.json) | [`gate-input-accounting_admin.json`](pass-2/gate-input-accounting_admin.json) |
| Accounting/admin gate results | [`p1-gate-accounting_admin.json`](pass-1/p1-gate-accounting_admin.json) | [`p2-gate-accounting_admin.json`](pass-2/p2-gate-accounting_admin.json) |
| Services judgment inputs | [`gate-input-services.json`](pass-1/gate-input-services.json) | [`gate-input-services.json`](pass-2/gate-input-services.json) |
| Services gate results | [`p1-gate-services.json`](pass-1/p1-gate-services.json) | [`p2-gate-services.json`](pass-2/p2-gate-services.json) |
| Venues judgment inputs | [`gate-input-venues.json`](pass-1/gate-input-venues.json) | [`gate-input-venues.json`](pass-2/gate-input-venues.json) |
| Venues gate results | [`p1-gate-venues.json`](pass-1/p1-gate-venues.json) | [`p2-gate-venues.json`](pass-2/p2-gate-venues.json) |
| Selected finding patch checks | [`patch-checks.json`](pass-1/patch-checks.json) | [`patch-checks.json`](pass-2/patch-checks.json) |
| Distinct raw patch checks | Included in pass-1 candidate and decision evidence | [`raw-patch-checks.json`](pass-2/raw-patch-checks.json) |
| Completeness and account-map signer leads | [`completeness.txt`](pass-1/completeness.txt) | [`completeness.txt`](pass-2/completeness.txt), [`map-dispositions.txt`](pass-2/map-dispositions.txt) |
| Program-name normalization | [`canonical-program-aliases.json`](pass-1/canonical-program-aliases.json) | Canonical keys retained in decisions |
| Final reading coverage | [`coverage.json`](pass-1/coverage.json) | [`coverage.json`](pass-2/coverage.json) |

The patch text is embedded in the reports, raw candidates and judgment artifacts.
Historical temporary patch filenames in check output are provenance, not required files.
The 19-raw-patch count belongs to the pass-2 check set; it is not a count of applied fixes.

## Specialist reports and reading records

| Lens | Pass 1 report | Pass 1 reading record | Pass 2 report | Pass 2 reading record |
|---|---|---|---|---|
| Math precision | [Report](pass-1/p1-01-math.md) | [Coverage](pass-1/p1-01-coverage.txt) | [Report](pass-2/p2-01-math.md) | [Coverage](pass-2/p2-01-coverage.txt) |
| Access control | [Report](pass-1/p1-02-access.md) | [Coverage](pass-1/p1-02-coverage.txt) | [Report](pass-2/p2-02-access.md) | [Coverage](pass-2/p2-02-coverage.txt) |
| Economic security | [Failure and unjudged partial note](pass-1/agent-03-failure.txt) | No completed record | [Report](pass-2/p2-03-economic.md) | [Coverage](pass-2/p2-03-coverage.txt) |
| Execution trace | [Report](pass-1/p1-04-execution.md) | [Coverage](pass-1/p1-04-coverage.txt) | [Report](pass-2/p2-04-execution.md) | [Coverage](pass-2/p2-04-coverage.txt) |
| Invariants | [Report](pass-1/p1-05-invariant.md) | [Coverage](pass-1/p1-05-coverage.txt) | [Report](pass-2/p2-05-invariant.md) | [Coverage](pass-2/p2-05-coverage.txt) |
| Periphery | [Report](pass-1/p1-06-periphery.md) | [Coverage](pass-1/p1-06-coverage.txt) | [Report](pass-2/p2-06-periphery.md) | [Coverage](pass-2/p2-06-coverage.txt) |
| First principles | [Report](pass-1/p1-07-first-principles.md) | [Coverage](pass-1/p1-07-coverage.txt) | [Report](pass-2/p2-07-first-principles.md) | [Coverage](pass-2/p2-07-coverage.txt) |
| Asymmetry | [Report](pass-1/p1-08-asymmetry.md) | [Coverage](pass-1/p1-08-coverage.txt) | [Report](pass-2/p2-08-asymmetry.md) | [Coverage](pass-2/p2-08-coverage.txt) |
| Account validation | [Report](pass-1/p1-09-accounts.md) | [Coverage](pass-1/p1-09-coverage.txt) | [Report](pass-2/p2-09-accounts.md) | [Coverage](pass-2/p2-09-coverage.txt) |
| Numerical gap hunter | [Report](pass-1/p1-10-numerical.md) | [Coverage](pass-1/p1-10-coverage.txt) | [Report](pass-2/p2-10-numerical.md) | [Coverage](pass-2/p2-10-coverage.txt) |
| Trust gap hunter | [Report](pass-1/p1-11-trust.md) | [Coverage](pass-1/p1-11-coverage.txt) | [Report](pass-2/p2-11-trust.md) | [Coverage](pass-2/p2-11-coverage.txt) |
| Flow gap hunter | [Report](pass-1/p1-12-flow.md) | [Coverage](pass-1/p1-12-coverage.txt) | [Report](pass-2/p2-12-flow.md) | [Coverage](pass-2/p2-12-coverage.txt) |

## Archive integrity and paths

All 90 evidence files are unchanged copies from the completed run
`.rust-auditor/runs/20261018-110553/`, with only `full-report.md` renamed to `report.md`.
The assembled report's SHA-256 is
`0d50668a2b76da361b44c357ae18fdff291fb1811e406a40f248f9d8a4a7e8d7`.
Run `sha256sum --check SHA256SUMS` from this directory to verify the archive.

The new indexes use relative links. Original machine paths, temporary paths, source line
numbers and claims remain unchanged inside historical artifacts. Interpret source paths
from the [repository root](../../../) at the audited commit. For an original run-artifact
path, drop the `.rust-auditor/runs/20261018-110553/` prefix and resolve the remainder from
this directory, applying the report rename above. Some raw references point to prior
project documents rather than files in this archive; those remain in the repository.

The twelve large transport source bundles, live auditor memory and the empty
`memory-before.tsv` are not archived. The source hashes identify the frozen source;
[`provenance.json`](provenance.json) records the empty starting ledger and bundle hash.
