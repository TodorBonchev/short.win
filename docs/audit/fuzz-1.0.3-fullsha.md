# Draft 1.0.3: final full-digest and current-venue fuzz run

**In progress; the ≥24-hour gate is not complete.** Source candidate
`e04a6776f1c2f5e02e581d76dd1255c416270275` includes the complete SHA-256 VenueGuard,
AI3-FR01 quarantine, retained keeper/browser recovery fixes and the reviewed supported
SOL/WSOL paths on the newly observed pump/PumpSwap builds.

Started **2026-10-20 22:21:09 UTC** (October 21 local).
Command: `shortwin-fuzz run --profile all --duration 24h --jobs 2 --actions 400
--seed 103410000 --out RESULTS --minimize`. All selects mock, mixed, curve,
PumpSwap, AMM v4 and CPMM.

| Frozen input | SHA-256 | Bytes |
|---|---|---|
| Release shortwin.so | `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc` | 1,644,024 |
| Test-venue shortwin.so | `0a405deba60a521798820985cd4cec1a0e61e46ed9d0ee829fd037b010565a01` | 1,675,056 |
| Mock CPMM | `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9` | 281,240 |
| Current-profile fuzzer runner | `5b6971e3fd11f8c8d3166d010631b40e0dd57b797f311ba97a348a35e1036b48` | 11,686,272 |

[Launch metadata](fuzz-1.0.3-fullsha-run.json) records paths, explicit input environment,
build flags, source revision and source archive digest. The frozen run directory is
`/root/fuzz/ready-103-20261021-e04a6776` in local WSL. It retains `source.tar`, copied
programs/runner, a separate read-only copy of all **334 tracked fixture files**, per-file
hashes, `running.json`, `run.log`, results and a supervisor which writes actual exit,
elapsed time and final summary/log hashes to `completion.json`.

`SHORTWIN_FIXTURES` explicitly selects the copied fixture directory; all three program
paths also select frozen copies. The metadata and actual running process environment
were checked. SHA256SUMS validates the copied fixtures. A separate agent independently
matched every copied file and all four artifacts with commit `e04a6776`, reproduced the
Git archive hash and reconstructed both new ProgramData accounts.
Source archive SHA-256: `caf9ce12131db852fea0e21b179fb997e1a731ed89933957c31a3e2715770840`.
Fixture manifest SHA-256: `f0d87f1c201b2df8016eb31648c533dc50f1832243ba84f16fdfe400166ddc4b`.

The profile is `PumpDeploy::Oct2026QuoteFees`: pump deploy **458,921,440**, PumpSwap
**458,921,517**, unchanged pump-fees **456,132,171**. The coin/pool state snapshots
retain their documented historical identities; this is not a coherent live mainnet bank.
[The venue review](live-preflight-1.0.3-2026-10-21.md) records the observed code bytes,
actual deployment times and supported-path compatibility boundaries.

The new runner was built with Rust 1.98.1, release LTO disabled and 16 codegen units;
two build jobs. All 12 harness self-tests and 21 release smoke/corpus tests passed on
the new source/profile before launch. Canonical platform-tools v1.57/SBPF-v3 rebuilds
prove the new unused baseline constant leaves both short.win program ELFs unchanged.
[Exact-ref full CI 39509811932](https://github.com/TodorBonchev/short.win/actions/runs/39509811932)
passed on `ff85a1b0` (25 successful jobs, zero failures, one intentional skip;
all 19 selected mutants killed), a test-only strengthening of AIR2-01. All 2,064 other tracked
entries match `e04a6776`; the standalone math test is not a runner input. Independent
source and actual frozen-artifact comparison confirms the unchanged production/fuzzer/
fixture inputs. The active 24-hour run continues. Final CI is green, verifier/Anchor/frozen bytes
match and both downloaded artifact ZIP digests are verified.

## Dated intermediate checkpoint

The [retained running checkpoint](probes/ci-fullsha-2026-10-21/local-fuzz-running-checkpoint.json)
and exact compressed log prefix record 6245 seconds, 2,776 runs,
531,463 actions and 6,310,502 transactions with **zero failures/anomalies**.
The actual frozen runner process was alive and had no completion record at observation.
This is partial evidence; all-profile coverage, ≥24-hour duration, zero exit and final
summary/log hashes remain pending. The later test/documentation-only commits do not
change the frozen inputs or restart the run.

## Superseded evidence

The first full-digest run at `a130fd98` was intentionally stopped after **2,827.80 s**
when the venue profile changed. Its last checkpoint at 2,822 s reported 1,105 runs,
208,883 actions, 2,482,211 transactions and zero failures/anomalies. Its
[metadata and actual stop record](fuzz-1.0.3-fullsha-previous-profile.json) are retained;
its duration is not credited to the new runner.

The earlier [13h13m FR01 run](fuzz-1.0.3-fr01.md) was intentionally superseded because
the full-digest correction changed program bytes. The still earlier
[completed clean run](fuzz-1.0.3.md) also uses different program bytes. None satisfies
the present final-profile ≥24-hour gate.

Before crediting completion, verify actual elapsed duration ≥86,400 s, process exit
zero, all-profile coverage, empty invariant failure/anomaly records, frozen input
hashes and final summary/log hashes. A launch or intermediate zero count is not
completion. The owner-authorized AI review route does not waive this requirement.
