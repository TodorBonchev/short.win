# Six-profile scheduling witness for the new qualification source

Source: `4c26706cc60b6fce9a2f2eb0e93dd824c2575e91`.
Later head `c865c38b297ec072d5a3d6dde25fb79e75ed950f` changes documentation,
retained evidence and licensing only; the five scheduler/counting sources below are
byte-identical at both commits and in the inspected working tree.

This is a read-only derivation. No new fuzz process was launched. Host choice and
new freeze metadata remain pending. The launch specification to verify against the
real process and frozen metadata is:

```text
bin/shortwin-fuzz run --profile all --duration 24h --jobs 2 --actions 400 --seed 103410000 --out BASE/results --minimize
```

The parser selects the exact ordered list
`mock,mixed,curve,pumpswap,amm-v4,cpmm`. It parses 24h as 86,400 seconds.
Differential checks and known-issue avoidance remain enabled by default. Do not add
a runs cap or different profile ordering/seed/jobs without recalculating this witness.

Worker `j` executes serial seeds `103410000 + j + 2*k` for zero-based `k`.
The profile index is `splitmix64(seed) % 6`; all arithmetic wraps as u64.
Main performs setup preflight for each selected profile before starting the shared
wall timer. Those preflight outcomes are discarded from summary counts. Each normal
outcome contributes exactly one run after successful setup; only completed outcomes
are merged. Zero setup/action/wind-down failures are essential: otherwise a worker
can consume a seed without contributing the successful prefix assumed here.

| Profile | First worker0 completed run / seed | First worker1 run / seed | Combined completed runs sufficient |
|---|---|---|---:|
| mock | 9 / 103410016 | 2 / 103410003 | 10 |
| mixed | 10 / 103410018 | 20 / 103410039 | 29 |
| curve | 17 / 103410032 | 5 / 103410009 | 21 |
| pumpswap | 2 / 103410002 | 17 / 103410033 | 18 |
| amm-v4 | 1 / 103410000 | 1 / 103410001 | 1 |
| cpmm | 3 / 103410004 | 4 / 103410007 | 6 |

If profile p is absent, worker0 and worker1 can have completed at most
`first0(p)-1` and `first1(p)-1` normal runs. Therefore
`first0(p)+first1(p)-1` guarantees p. The maximum is 29, for mixed:
9+19=28 completed runs can still miss it. All 30 possible worker-prefix splits
whose total is 29 were also checked explicitly. Thus **29 successful completed
normal runs guarantee that the union selected all six profiles**, regardless of
worker speeds.

Worker0's first 17 and worker1's first 20 each individually cover all six.
If neither worker completed its whole all-six prefix, total runs are at most
16+19=35. **36 successful completed runs** is the conservative sufficient bound
that also guarantees at least one worker completed its own all-six prefix.
It is valid to keep using 36 operationally.

Independent calculation: a Python implementation with explicit u64 masking and a
standalone Rust program importing the exact source-pinned `rng.rs` agree on all 80
draws (40 per worker), including full 64-bit hashes, seeds, indices and names.
The Rust program is only a scheduling arithmetic check: it does not execute the
fuzzer, Token-2022 or short.win. `python-schedule.json`, `rust-schedule.csv`,
`verify-schedule.rs` and the copied exact sources retain the calculation.

A final qualifying run still requires authenticated binary/source/fixture inputs
and the actual command above, readable final completion and summary, supervisor
elapsed and summary wall_seconds each at least 86,400, zero failure counts, empty
failures and anomalies, no crash/min files, exact final summary/log hashes and
natural completion of in-flight work. Expected refused transactions need not be
zero. Exit 0 alone is insufficient. The summary has no per-profile totals: this
witness establishes selection of each profile, not equal allocation, explicit
profile counters or 24h per profile. Instruction/event/error telemetry remains
separate observed coverage evidence.

The old witness in
`/root/short.win-audit-review/target/fuzz-monitor-verification-notes.md`
gives the same result. All five scheduler/counting source blobs are also
byte-identical to its e04a6776 source; their old fixture/harness/artifact identity
and completed duration are NOT credited to this new baseline.

| Source file at 4c26706 | SHA256 |
|---|---|
| `tests/fuzz/src/main.rs` | `0a43ed1e44dd503e0b2dd99abb3471efcfda50d945de42ce2a3deeb7cd9f8380` |
| `tests/fuzz/src/action.rs` | `ec152b9247264245c5d4122d3c5bc107b4d538b25d95c0eee2c5f560915969ce` |
| `tests/fuzz/src/rng.rs` | `5b7bb375bcc55ec66650278f433ad2dfda2839e7928ae3fe7ea22895b59c8943` |
| `tests/fuzz/src/runner.rs` | `a4478db031c023299fa15e907e8e2ad1add094a607e3070d4da234830e0f84be` |
| `tests/fuzz/src/stats.rs` | `e1e53085da36f0784fee23a7161381ee3f8e9bc7f651086cbcd4593c3984dc7c` |

`witness.json` retains exact Git blob IDs, line anchors, launch assumptions,
minimality counterexample, old-witness hash and all first-occurrence details.
