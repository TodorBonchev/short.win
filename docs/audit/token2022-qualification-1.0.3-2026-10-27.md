# Token-2022 11.1 qualification candidate — 2026-10-27

**Engineering CI follow-up is complete; full release qualification is not.
Replacement PR CI passed 24 jobs with two intentional skips, and the original
source-equivalent dispatch killed all 19 selected mutants. The fresh 24-hour
campaign is NOT STARTED and is on hold: the owner postponed mainnet while the
Jupiter redesign scope is decided. No long-run host choice is required for this
pre-redesign baseline. No final tag or mainnet transaction has been made.** This batch
follows the independently reviewed source/provenance disposition in the
[mainnet preparation record](probes/mainnet-preparation-2026-10-27/README.md).
The owner-selected sequence remains mainnet before external audit/funded bounty,
with existing per-market borrowing limits and uncapped deposits/market count.

## Candidate inputs and preserved history

The new dated `spl_programs_2026_10_27` fixture group selects the observed
Token-2022 `program@v11.1.0` deployment at slot **460,497,271**, ELF SHA-256
`3dde54d202caa2d10514fd58a154df5f26f07643f9f5fd0402ff08084f0cb954`, trimmed SHA-256
`5e888b294bd1941bfc47107a73d5e40bb24fde0d639f69099e7920ad8b6413d9`.
The old 11.0 fixture, old manifest objects/snapshots and independent old loader
identity remain intact. No short.win on-chain adapter, math or account schema
changes are proposed. A new fixture baseline does not inherit the old run's time.

The synthetic execution bank starts at **460,497,272**, one slot after the new
upstream deployment. Historical snapshot slots translate by an explicit fixed
**10,497,272** offset. Raw current-bank arithmetic/restoration remains raw;
there is no clamp or automatic double translation. Fixture ProgramData, Pyth
posted slots and historical Unix timestamps are not rewritten. These mixed
historical-state/new-code runs are controlled integration evidence, not a claim
that the full account set was an actual current-mainnet bank snapshot.

The executed regressions check old/new code at the same bank, repeated cache
replacement through a synthetic Memo-v4-ID acceptance tripwire, plain mints,
all six admitted metadata/group extensions and their combination, actual
InitializeAccount3/ATA sizing, 165-byte pool vaults, 170-byte ImmutableOwner ATAs,
malformed/unknown TLVs, and precise clock/oracle slot behavior. The old measured
2,487-CU memo control remains old-only. Passing tests enforce the new ELF's
positive transfer execution and existing 100,000-CU withdrawal budget. The
ordinary passing CI output suppresses the numeric memo/transfer diagnostic;
no exact new transfer-CU fingerprint is claimed retained. Other instruction
measurements are retained below; no equal-CU assertion is invented.

## Build-dependency and artifact preparation

Root and five affected benchmark lockfiles patch only ctutils **0.4.2 → 0.4.4**
with its published checksum. The root RUSTSEC-2026-0385 exception is removed from
policy, CI audit flags and expiry metadata. Services already use patched ctutils
and Tokio. The remaining independently documented usage-scoped dispositions and
November 27 deadline remain; no broad warning suppression is introduced.

CI additionally builds and retains `shortwin-fuzz-runtime`: four actual binaries,
source commit/tree and archive, toolchain/build settings, all dynamically
enumerated tracked fixture hashes, source-archive cross-checks and SHA256SUMS.
That artifact is emitted only after the job's smoke succeeds; its existence is
not a full-gate or 24-hour result. No campaign is started for this held baseline.
Any future qualifying campaign must use its selected reviewed inputs and preserve
supervisor/process identity, actual all-profile command,
completion, readable summary, exact log bytes/hashes, zero invariant failures and
zero anomalies, and at least 86,400 seconds in supervisor and runner records.
Allow in-flight runs to finish naturally; do not restart or combine durations.
Retain a newly checked profile-scheduling witness without inventing per-profile
duration or counters.

## Terminal engineering results and browser repair

The [terminal CI receipts](probes/token2022-qualification-2026-10-27/ci-40443684567/terminal-independent-verification.json)
retain both runs' actual terminal APIs, raw relevant logs and source comparison:

- [CI 40440067371](https://github.com/TodorBonchev/short.win/actions/runs/40440067371),
  `workflow_dispatch` on `4c26706cc60b6fce9a2f2eb0e93dd824c2575e91`, finished
  **failure: 24 successful jobs, one browser failure, one intentional nightly
  skip**. Its program tests and mutation job completed successfully. All **19
  unique selected mutants** were killed by their named tests; seven were also
  killed by three-minute profile fuzz, and twelve were tests-only. Duplicate
  summary lines are not counted as additional mutants.
- [CI 40443684567](https://github.com/TodorBonchev/short.win/actions/runs/40443684567),
  `pull_request` on `658d8baafca01d751452d1c67b508747fe0671e2`, finished
  **success: 24 successful jobs and two intentional workflow skips**, mutants
  and fuzz-nightly. Those jobs run only under their configured dispatch/nightly
  conditions. This PR run does not itself claim a mutation pass.

The combined evidence is source-equivalent for program, fuzzer, fixtures,
lockfiles, services, production web/client code and CI workflow inputs. From
`4c26706` to `658d8baaf`, only one browser test plus documentation/evidence/licensing
changed. The following `3fa558a1b` changes documentation, measured benchmark
reports and five measured cost-model constants; it does not change those runtime
inputs. This is an explicit combination of two observed runs, **not a claim that
one repaired manual run passed 25 jobs**. A later source-comment correction only
clarifies memo diagnostic visibility; it changes no assertion or instruction.

The failed original browser job passed 124 tests and skipped two; the positions
countdown assertion failed on both attempts. Its seeded chain position opened
at Unix 1,793,194,441 with deadline 1,793,799,241, while browser wall time was about
1,793,070,100: the newly warped validator bank clock was roughly 34.5 hours ahead
of the host browser. The seven-day upper bound correctly rejected the resulting
729,141-second display. The focused test repair aligns that browser with the
actual validator Clock before navigation, retaining a progressing clock and the
original duration/countdown assertions. Production countdown logic and timeout
bounds are unchanged. The repaired positions test actually passed in 5.9 seconds.
The main browser mode passed **125 tests with two opposite-mode skips**; the
flag-off mode passed **nine with nineteen flag-on skips**. These are execution
counts under different feature modes, not unique-test or mainnet execution counts.

The replacement program-tests log explicitly passes all eleven targeted controls:
raw and historical clock behavior, old/new base-transfer compatibility,
all-six/combined admitted-extension pool-vault/ATA sizing, malformed/unknown TLV
sizing rejection, both authenticated fixture pins, old/new memo-required
withdrawals, the sibling-memo negative control and repeated old/new synthetic
Memo-v4-ID cache replacement. No targeted Token-2022 control was skipped. Five
other opt-ins remain ignored: the Surfpool fork, exploratory liquidation grid,
R-PFEE withdraw probe and two pre-M4R external-build locksteps. Their absence is
not represented as qualification coverage. The ordinary test command omits
`--nocapture`, so successful memo diagnostics do not expose an exact new transfer
CU; the passing assertions enforce the historical fingerprint and new budget.

Independent artifact checks verified all three GitHub ZIP digests, exact runtime
checksums, all 2,533 archived source blobs and all **337 tracked fixture files**.
Anchor, verifier and bundled release bytes remain the canonical
`d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc`; the new native
fuzzer is `3ffb12b803f01c1cc60bd82bcc386834e8f148ac0e8fb934df533882953769c3`.
The 346 managed fixture entries and 337 tracked files count different things.
The new reviewed live baseline passed at slot **461,913,288** on October 27; this
is a dated observation and must be repeated before final financial execution.

Thirteen generated benchmark reports were refreshed from exact successful CI
diffs, matching every before/after Git blob and independent reconstruction review.
The [retained measurements](probes/token2022-qualification-2026-10-27/benchmarks/reconstruction.json)
cover all six affected bench jobs; all 254 printed instruction cases stayed
within existing budgets with no Mollusk/LiteSVM mismatch. For example, the new
Token-2022 create/deposit measurements are 54,384/36,597 CU; the CPMM normalized
open is 132,447 of its 150,000-CU budget. These observed deltas are not all
attributed solely to the upstream upgrade. No depth report changed. The
[keeper-cost model output](probes/token2022-qualification-2026-10-27/keeper-cost/keeper_cost.txt)
was refreshed from five measured inputs and cross-checked; its functions, fees
and economic/default assumptions remain unchanged. Historical ECON tables are
explicitly qualified by the dated new-output note. None of these model figures
establishes actual funded operation or guaranteed future fee revenue.

The [new scheduling witness](probes/token2022-qualification-2026-10-27/scheduling/WITNESS.md)
checks the exact source and Python/Rust arithmetic. With the stated seed and two
workers, 29 successful completed ordinary runs suffice to infer that all six
profiles were selected; the conservative 36-run bound additionally guarantees
one worker completed its whole all-six prefix. No new launch/process identity or
per-profile duration is claimed. Reusing the historical seed requalifies the new
baseline; it is not presented as independent extra seed coverage.

## Held release gates after the architecture decision

The engineering results above are complete for this candidate's reviewed
Token-2022 inputs. They are not a final mainnet approval. The owner postponed
mainnet while considering a Jupiter redesign; no redesign implementation is
approved or qualified by this record.

1. Select the intended architecture and release scope before choosing a final
   runtime baseline. Reassess every changed program, harness, fixture, dependency,
   client and keeper input; these receipts cannot qualify untested redesign code.
2. The new-baseline all-profile ≥24-hour campaign is **NOT STARTED**. It is held,
   not failed or completed, and no PC/VPS host selection is currently required
   for this pre-redesign baseline. Preserve the old run with its original source,
   fixtures and duration; do not borrow that duration for changed inputs.
3. Before any future final tag/signing packet, repeat current live dependency
   identities, complete the selected baseline's qualifying campaign and checks,
   and verify actual funded operator configuration/monitoring. No final tag,
   deployment, financial transaction or funded-operation evidence follows from
   the successful automated checks here.
