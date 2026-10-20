# Draft 1.0.3: completed 24-hour fuzz gate

**Scope after the 2026-10-20 integration:** this completed run covers the
preceding `9b91060` candidate and its hashes below. The later AI3-FR01 quarantine
changes release and test-venue program bytes. A new ≥24 h run on those final
bytes is required; this historical clean result does not close that gate.

The all-profile run against the unreleased 1.0.3 candidate completed on 2026-10-19.
It began at 2026-10-18 13:44:14 UTC and ran for 86,405.2476 seconds (24 hours
and 5.25 seconds). Its final [raw summary](fuzz-1.0.3-summary.json) records
73,349 runs, 13,967,758 actions, 166,090,860 transactions, 301,021,313 hook
checkpoints and 73,349 wind-downs. `failures` and `failure_counts` are empty,
and `anomalies` is empty. The runner wrote its final summary and log footer,
then exited; the detached launcher did not record a separate shell exit code.

The runner used `--profile all`, `--duration 24h`, `--jobs 4`,
`--actions 400`, `--seed 103000000` and `--minimize`. `all` selects mock, mixed, curve, PumpSwap,
AMM v4 and CPMM. The deterministic schedule exercised every profile. The
summary's coverage counters report 28 of 30 instructions landed and 68 of
110 error codes returned. `transactions_failed` is 18,433,991: these are
transaction refusals exercised by the scenarios, not invariant-fuzzer failures.

The frozen binaries copied into the run match the exact-ref CI candidate
`9b910602ed076a6d4359c496960b3ffc0e7c6af8`:

| Artifact | SHA-256 |
|---|---|
| Release `shortwin.so` | `b88aa520c24301522b45871e73622a4d498603dc03a8f34ff8bbb557d7fa0647` |
| Test-venue `shortwin.so` | `994ad12c468f465937af21080ec3c48678bb1f197ab143ab44f91f98ca56132b` |
| Mock CPMM `mock_cpamm.so` | `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9` |
| Local `shortwin-fuzz` runner | `8a2515520356b7150e15bfc6c9785a4433f0e17c155deccd0e4f14621a856b52` |

The first three hashes match the [CI artifact record](ci-1.0.3-hashes.txt).
The local runner matches the hash captured at launch; CI did not publish a
runner-binary hash. The fuzzer source matches the candidate commit.

For local evidence, `target/fuzz-103-20261018/results/summary.json` has SHA-256
`f3521e05e6054438cac73afae106d6f57838972e3b99161715f466fc32a9ed8c`;
the copied summary above has the same hash. The final `run.log` has SHA-256
`5f089b5be8a1c707648a8c957aa872892581f3f6b903c34f8a6e9b2e03abb78a`.
Its final sections say `anomalies: none` and `failures: none`. The raw log and
run metadata remain in `target/fuzz-103-20261018` on the runner machine.

This closes the fuzz gate for the identified `9b91060` candidate only.
[Independent fix review](ai-review-3/fix-review-1.0.3.md)
and written disposition of the remaining open findings are still required
before marking fixes final or tagging a release.
