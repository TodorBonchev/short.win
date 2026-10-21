# Full-digest candidate CI evidence — 2026-10-21

[Final full CI 39509811932](https://github.com/TodorBonchev/short.win/actions/runs/39509811932)
completed **success** on exact test source `ff85a1b0c8452cb9fad32767f7083d22377e90a3`,
attempt 1: **25 jobs passed, zero failed, one intentional `fuzz-nightly` skip**.
The completed record includes all 26 job identities/times and 332 step states, with no
failed step. This credits the full-CI gate; the local current-profile ≥24-hour fuzz
and first-deployment operating requirements remain separate.

- [Final job/step summary](final-ci-summary.json) and [raw API retention index](final-api-retention.json)
  identify the exact completed run. The raw run/jobs responses are retained losslessly
  as gzip, with original and stored hashes.
- [Program-suite evidence](final-program-tests-evidence.json), [actual log](final-program-tests-job.log.gz)
  and [retention hashes](program-log-retention.json) record the completed core suite.
  Main scenario execution took about 74 minutes; normal summaries and cleanup confirm
  successful completion, including both upgrade cases.
- [Artifact metadata](final-artifacts-metadata.json) and [build-log retention index](build-log-retention.json)
  retain raw Anchor/verifier logs and the verifier hash file losslessly. SBF files/ZIPs
  remain in the local target/frozen run directories; recorded hashes identify them.
- [Actual mutation summary](final-ff85-mutants-evidence.json) verifies the exact source's
  19 selected names against the completed new-run log: 19 test kills, seven required
  positive-failure fuzz kills and twelve explicitly test-only checks; no survivors.
  [Raw log](final-ff85-mutants-job.log.gz) and [retention hashes](mutants-log-retention.json).
  These expected failures run against deliberately mutated programs, separately from
  the unmodified candidate's long fuzz run.
- [Final ff85 verifier/Anchor comparison](final-ff85-verifier-comparison.json) verifies
  actual raw byte equality and both downloaded ZIP digests. The comparison's own dated
  snapshot was made before the complete workflow finished; its identity checks remain
  valid and the completed full-CI result is recorded above.
- [Separate strengthened-regression execution](air2-01-test-peer-execution.log.gz) and
  [raw/stored hashes](peer-log-retention.json) retain the independent peer's exact
  one-passed/15-filtered result.
- [Partial local fuzz checkpoint](local-fuzz-running-checkpoint.json) and
  [exact log prefix](local-fuzz-running-log-prefix.log.gz) record an intermediate zero
  count on the frozen final profile. They do not establish ≥24-hour completion,
  all-profile coverage or final summary/exit hashes.
- [Main observation](main-observation.json) records the prior main checkpoint and its
  successful FR01 source CI. The new candidate remains on its audit branch pending
  the separate release gates.
- [Superseded run outcomes](superseded-runs.json) retain job IDs, conclusions and counts.
  Both runs ended cancelled; neither supplies the final completed full-CI gate.
- [Superseded e04 verifier comparison](superseded-e04-verifier-comparison.json) and
  [Anchor comparison](superseded-e04-anchor-comparison.json) retain earlier actual-byte
  checks. Their partial workflow results are not credited as green.
- [Independent e04-to-ff85 source equivalence](source-equivalence-e04-ff85.json) confirms
  the only tracked change is the strengthened standalone math regression. All 2,064
  other entries, 14 checked production/fuzzer/fixture/config scopes, six recorded build
  inputs and four actual frozen artifacts match. This equivalence does not prove
  completed fuzz coverage; publishing later documentation changes no runtime input.

From this directory, run `sha256sum -c SHA256SUMS`. The accurately labelled AI reviews
and delegated owner decisions are separate from execution results. No deployed program,
official external-firm report or completed 24-hour run is claimed.
