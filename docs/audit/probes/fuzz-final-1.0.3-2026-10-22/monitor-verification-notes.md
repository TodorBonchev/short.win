# Frozen 1.0.3 fuzz monitor verification notes

Read-only inspection by the AI `keeper_fix_review` agent on 2026-10-21. This is operational verification guidance, not an external firm's acceptance or release/deployment approval. No tracked files or running processes were modified.

## Run and current observation

Base: `/root/fuzz/ready-103-20261021-e04a6776`.
Source: `e04a6776f1c2f5e02e581d76dd1255c416270275`.
Started: `2026-10-20T22:21:09.024380+00:00`.
Supervisor PID `150246`; runner PID `150247`.
Command: `bin/shortwin-fuzz run --profile all --duration 24h --jobs 2 --actions 400 --seed 103410000 --out BASE/results --minimize`.

Both processes were alive; the runner's loaded `/proc/150247/exe` bytes, frozen executable, cwd, complete argv and four selected input environment variables match launch metadata. The latest checkpoint captured in `target/fuzz-monitor-current-identity.json` is 30,200 seconds, 13,369 runs, 2,546,966 actions, 30,490,119 transactions, zero failures and zero anomalies. The health record SHA-256 is `e29da8b6df03dc530402d0e9071ab83b21311cc3f2f21dc151177e169033c171`. Its counters are a dated checkpoint, not final totals.

All four frozen artifacts and all 334 fixture files were independently rehashed and match their records. Current HEAD (`f894b810` at inspection) has no changes from the launch source in program, math/venue source, fuzzer, shared harness or fixtures. Frozen `source.tar`, manifest, checksums and per-file record hashes also match launch metadata.

## Immutable identity anchors

Require these fixed hashes, rather than trusting a possibly changed metadata file's own claims:

| File relative to BASE | SHA-256 |
|---|---|
| `metadata.json` | `3cb472ecc4a911394e4e6cbe40299ca6486a9f780c6790d87bf8ff57bc772e4c` |
| `running.json` | `85f3347ab8931f629d1cd2c14b6af461f0adb0f2c09638724ec7d5fb917e3aec` |
| `supervise.py` | `e69c1f8f773c4400debe11dd6430369be0b8c927a264f81cbe81e156cb83a9d1` |
| `bin/shortwin-release.so` | `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc` |
| `bin/shortwin-test-venue.so` | `0a405deba60a521798820985cd4cec1a0e61e46ed9d0ee829fd037b010565a01` |
| `bin/mock_cpamm.so` | `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9` |
| `bin/shortwin-fuzz` | `5b6971e3fd11f8c8d3166d010631b40e0dd57b797f311ba97a348a35e1036b48` |

Pinned metadata contains the expected byte sizes, source archive SHA-256 `caf9ce12131db852fea0e21b179fb997e1a731ed89933957c31a3e2715770840`, fixture manifest/checksum-record hashes, 334 per-file expectations and six build-input hashes. Recheck actual bytes against them at completion. Do not combine duration with either superseded run.

## Running-state interpretation

- Read `run.log`, `running.json`, `supervisor.log` and validated process identities. Inspect only the four named input environment keys; do not dump the entire process environment.
- A missing `results/` directory or `results/summary.json` is normal during a clean run. `main.rs:439–441` creates the directory and writes the summary at shutdown; `runner.rs:276–292` creates it earlier only for a crash.
- The 24-hour deadline is **global**, after six-profile preflight; it is not 24 hours per profile. At the deadline, workers finish their in-flight scenarios and wind-downs before writing the summary. Do not kill a healthy run at 24 hours.
- A positive invariant-failure or anomaly checkpoint is actionable. Preserve logs/crash files and report it; do not erase, restart, change the input bytes or weaken checks.
- If a validated process disappears without completion, or progress/log freshness stops for several minutes, inspect and report the condition. A stale checkpoint is grounds to investigate, not evidence of a clean pass or automatic authority to restart.
- Remain quiet when the state is unchanged. Notify on meaningful failure, identity drift, unexpected termination or validated completion.

## Exact completion gate

The supervisor waits for the runner, closes the log, then writes `BASE/completion.json`:

```json
{
  "started_utc": "...",
  "finished_utc": "...",
  "elapsed_seconds": 86400.0,
  "exit_code": 0,
  "summary_sha256": "...",
  "log_sha256": "..."
}
```

The two hash fields are conditional in the supervisor: missing files omit them. Therefore their presence and agreement must be required. The files are not written atomically; briefly retry a read that catches finalization in progress.

Before crediting the gate:

1. Require a complete, readable `completion.json` whose start matches `running.json`, exit is exactly zero, and monotonic `elapsed_seconds >= 86400`. Check finish/start timestamps for consistency. The earliest wall-clock 24-hour point is `2026-10-21T22:21:09.024380Z`; that timestamp alone is not completion.
2. Require readable `BASE/results/summary.json`. Require its own `wall_seconds >= 86400`, positive runs/actions/transactions/hook checkpoints/wind-downs, `failures == []`, `failure_counts == {}` and `anomalies == {}`. Require no `results/crash-*.json` or `results/min-*.json`. Inspect final textual failure/anomaly sections and any other result files.
3. Independently hash final summary and final raw `run.log` and match `completion.summary_sha256` and `completion.log_sha256` exactly. Preserve completion and its hash, summary and its hash, full raw log and its hash, and compressed-copy hashes if retained.
4. Rehash the fixed identity anchors and all 334 fixtures. Check final candidate program/fuzzer/harness/fixture source equivalence to the frozen inputs; keep launch source labeled `e04a6776`. Later docs or the standalone AIR2-01 test do not change runner inputs; later production/harness/fixture changes require a new scope assessment.
5. Validate six-profile selection using the witness below. Do not claim per-profile duration or per-profile totals absent telemetry.
6. Record the exact source, four artifact hashes, fixture profile, seed, command, elapsed times, exit, final counts, failure/anomaly emptiness, and final file hashes in the existing audit/fuzz records. Preserve historical records and existing economic-risk boundaries. Current exact-ref CI `39509811932` on `ff85a1b0` is already green and its Anchor/verifier/frozen identity evidence is retained in `docs/audit/probes/ci-fullsha-2026-10-21/`; do not rerun it merely because this fuzz finishes.

**Two important distinctions:** `cmd_run` returns failure only for recorded invariant failures, so an exit of zero can coexist with anomalies. Its summary write uses `.ok()`, so exit zero can also coexist with a missing/unwritten summary. Conversely, `transactions_failed` counts expected refused transactions and need not be zero; it is not the invariant-failure count.

After clean validation, update audit evidence and assess the remaining 1.0.3 release checks under existing authorization. Fuzz completion does not itself supply operator preflight, external-firm acceptance, a release tag or deployment authority.

## Six-profile selection witness without modifying the runner

The summary has no per-profile run counters. At `main.rs:359–367`, worker `j` completes a contiguous prefix of seeds `103410000 + j + 2*k` and chooses `Profile::ALL[splitmix64(seed) % 6]`. `action.rs:344–361` fixes order `mock,mixed,curve,pumpswap,amm-v4,cpmm`; `rng.rs:5–10` fixes SplitMix64. `runner.rs:195` sets one completed run's counter to one; the main loop merges those counters. These source blobs match the launch source.

| Profile | First worker-0 run (1-based) / seed | First worker-1 run / seed | Total completed runs sufficient to guarantee selection |
|---|---|---|---:|
| mock | 9 / 103410016 | 2 / 103410003 | 10 |
| mixed | 10 / 103410018 | 20 / 103410039 | 29 |
| curve | 17 / 103410032 | 5 / 103410009 | 21 |
| pumpswap | 2 / 103410002 | 17 / 103410033 | 18 |
| amm-v4 | 1 / 103410000 | 1 / 103410001 | 1 |
| cpmm | 3 / 103410004 | 4 / 103410007 | 6 |

If a profile were absent, neither worker could have completed its first occurrence; their maximum combined completed runs would be `(first0-1)+(first1-1)`. Thus **29 completed normal runs** guarantee all six were selected, regardless of worker speed. A final clean summary with `runs >= 29`, unchanged two-worker command/seed/source/runner and no setup/invariant failures establishes this derived selection witness. It establishes neither equal allocation nor 24 hours for each profile. Detailed independently calculated witnesses are retained in `target/fuzz-monitor-current-identity.json`.
