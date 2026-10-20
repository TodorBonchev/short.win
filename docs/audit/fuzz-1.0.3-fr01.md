# Draft 1.0.3: superseded partial FR01 fuzz run

**2026-10-21 disposition:** intentionally stopped after 47,610.25 seconds when the
owner-selected full-SHA-256 correction changed program bytes. Its last checkpoint
had 20,935 runs and 47,729,011 transactions, zero failures/anomalies. Exit `-2`
records the requested interrupt, not an invariant failure. No final summary was
produced. [Stop/completion record](fuzz-1.0.3-fr01-partial.json);
[new final-byte run](fuzz-1.0.3-fullsha.md). This partial run does not close the gate.

Original launch record: started **2026-10-20 08:03:17 UTC** from source candidate
`b8183266655def0195ad18608f103d15ebc03ac6`, including the AI3-FR01 guard.
The runner is configured for at least 24 hours with all six profiles, two workers,
400 actions, seed `103200000`, and failure minimization. **The run was superseded before completion**;
this launch record does not close the fuzz gate.

The frozen run directory is `/root/fuzz/fr01-20261020-b8183266` on the local WSL machine.
It contains copied binaries, `source.tar`, `metadata.json`, `running.json`, `run.log`,
the result directory, and a supervisor which records process exit code and elapsed
time in `completion.json`. The copied [launch metadata](fuzz-1.0.3-fr01-run.json)
records the exact command, build settings, source archive digest and binary sizes.

| Frozen artifact | SHA-256 |
|---|---|
| Release `shortwin.so` | `eb9cd3e68c33de8119b11f978ab77f65d4fa41de79904c508f0fd9d519d737dd` |
| Test-venue `shortwin.so` | `cf1c44424ff1c879ac5639b432a25c0dc4a81c5842ce047e26668ef920030322` |
| Mock CPMM | `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9` |
| Fuzzer runner | `26de7dbf83edd78ec9439929f4ee963da296e7efc20b3f24b3f16dbbe3866c0c` |

The program/fuzzer input diff from integration `eae77d9a` through this source candidate
is empty: intervening changes affect keeper and frontend only. The runner was rebuilt
from the source candidate using Rust 1.98.1, release LTO disabled and 16 codegen units.
The program artifacts match the prior [FR01 verified-build record](ci-1.0.3-fr01-hashes.txt).
They are different from those in the [preceding clean 24-hour run](fuzz-1.0.3.md).

After the process exits, check elapsed duration, shell exit code, all-profile coverage,
failures and anomalies, rehash the frozen binaries, and retain the final summary/log
digests. Record the final result and match the integrated CI artifact hashes before
crediting the ≥24-hour gate. No AIR-01–AIR-24 waiver applies to this batch.
