# Incoming 56f660027 CI and indexer review - 2026-10-22 local

This is independent read-only source, metadata, raw-log and actual-artifact review by Codex agent remote_scope_check. It is AI-assisted evidence, not an external firm's audit, deployment authorization or proof of running operator service.

## Result and source boundary

CI [39556305501](https://github.com/TodorBonchev/short.win/actions/runs/39556305501) completed success on exact source 56f660027f88ee60a33b9d2b5da516daccc9c9b9: 24 successful jobs, zero failed jobs and two skipped jobs. The skips are fuzz-nightly and mutants. This was a push event; the workflow runs mutants only on workflow_dispatch or scheduled opt-in.

The prior ff85 CI's actual 19 mutant kills are separately attributed. Programs, crates, tests, root manifests/lockfile/toolchain and scripts/mutants.sh have identical Git objects between ff85 and 56. This is relevance/equivalence evidence, not a fresh mutant execution. The completed frozen e04 fuzz run remains separately identified; ff85's standalone accounting test addition and the incoming service/build-policy changes do not rename that run.

All three freshly downloaded Anchor SBF programs are byte-identical to the frozen programs; the new verifier release is byte-identical to both. New Anchor IDL is byte-identical to the current committed client IDL. Both downloaded ZIP hashes and sizes match GitHub artifact metadata bound to the new source.

- [Compact CI, source and actual-byte evidence](new-ci-review.json) contains all job/step outcomes, actual focused test log lines, source-object equality and artifact comparisons.
- [Indexer and deployment source review](indexer-source-review.md) records the narrow fix, existing regression strength, optional boundaries and remaining operating limits.
- run.json, jobs.json and artifacts.json retain the unmodified REST API responses.
- verifiable-hashes.txt retains the new verifier's source declaration.
- [Log retention map](LOG_RETENTION.json) records exact raw and deterministic gzip hashes. The .log.gz files preserve the original downloaded job/build logs without modifying their bytes.
- [Checksums](SHA256SUMS) cover the portable evidence files. ZIP files and uncompressed large logs remain only in ignored target storage and must not be copied into the tracked audit packet.

The actual checkpoint, destination-3, current-layout and browser/client tests ran in this new CI; no local full-suite rerun was performed or claimed. Code compatibility does not prove which deployed binary/bundle is running, payer funding, top-up arrangements or alert delivery. Those first-deployment prerequisites remain as recorded separately.
