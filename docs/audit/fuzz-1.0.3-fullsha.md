# Draft 1.0.3: final full-digest fuzz run

**In progress; the ≥24-hour gate is not complete.** Source candidate
`a130fd9836508e0f58bc94eb00bb1f5eae95405e` includes the complete SHA-256 VenueGuard correction
and the owner-selected policy disclosures (owner decision date: 2026-10-21).

Started **2026-10-20 21:34:01 UTC**.
Command: `shortwin-fuzz run --profile all --duration 24h --jobs 2 --actions 400
--seed 103400000 --out RESULTS --minimize`. All selects mock, mixed, curve,
PumpSwap, AMM v4 and CPMM.

| Frozen input | SHA-256 | Bytes |
|---|---|---|
| Release shortwin.so | `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc` | 1,644,024 |
| Test-venue shortwin.so | `0a405deba60a521798820985cd4cec1a0e61e46ed9d0ee829fd037b010565a01` | 1,675,056 |
| Mock CPMM | `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9` | 281,240 |
| Fuzzer runner | `35772ea152704e073e14e1a1b313483f3997f43c63dcee520c27fb387f6600cc` | 11,685,792 |

[Launch metadata](fuzz-1.0.3-fullsha-run.json) records exact paths, build flags,
source revision and source archive digest. The frozen run directory is
`/root/fuzz/fullsha-103-20261021-a130fd98` in local WSL. It retains `source.tar`, copied binaries, `running.json`,
`run.log`, results and a supervisor which writes the actual exit code, elapsed time
and retained-file hashes to `completion.json`. Rebuild used Rust 1.98.1, release
LTO disabled and 16 codegen units. Smoke/corpus and harness self-tests passed first.

The full-digest input diff from `f5a8ccef` to the candidate is empty for program,
math/venue implementation, fuzzer, shared LiteSVM code and Cargo manifests/lock.
The downloaded Anchor release and test-venue artifacts from
[CI 39501064777](https://github.com/TodorBonchev/short.win/actions/runs/39501064777)
match the frozen program inputs above; the CI verifier release is byte-identical too.
That run's services dependency failure still requires correction and fresh full CI.
Subsequent services-only changes must preserve the program/fuzzer source and root
Cargo inputs for this run to remain final-byte evidence.

The [preceding FR01 run](fuzz-1.0.3-fr01.md) was intentionally stopped after
47,610.25 seconds because the authorized digest correction changed program bytes.
Its last retained checkpoint reported 20,935 runs and 47,729,011 transactions with
zero failures/anomalies. It is partial evidence, not a completed ≥24-hour run.
The still earlier [clean run](fuzz-1.0.3.md) also uses different program bytes.

Before crediting completion, verify actual elapsed duration ≥86,400 seconds, process
exit zero, all-profile coverage, empty invariant failure/anomaly records, frozen input
hashes and final summary/log hashes. A launch or intermediate zero count does not
satisfy the gate. The owner-authorized AI-review route does not waive this requirement.
