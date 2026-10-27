# Token-2022 11.1 qualification candidate — 2026-10-27

**Preparation only: execution tests, fresh CI and the new 24-hour campaign are
pending. No final tag, mainnet transaction or long run has started.** This batch
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

The proposed regressions check old/new code at the same bank, repeated cache
replacement through a synthetic Memo-v4-ID acceptance tripwire, plain mints,
all six admitted metadata/group extensions and their combination, actual
InitializeAccount3/ATA sizing, 165-byte pool vaults, 170-byte ImmutableOwner ATAs,
malformed/unknown TLVs, and precise clock/oracle slot behavior. The old measured
2,487-CU memo control remains old-only. New compute is measured against the
existing withdrawal/program budgets; no equal-CU assertion is invented.

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
not a full-gate or 24-hour result. The qualification host must use those exact
inputs and preserve supervisor/process identity, actual all-profile command,
completion, readable summary, exact log bytes/hashes, zero invariant failures and
zero anomalies, and at least 86,400 seconds in supervisor and runner records.
Allow in-flight runs to finish naturally; do not restart or combine durations.
Retain a newly checked profile-scheduling witness without inventing per-profile
duration or counters.

## Remaining gates

1. Obtain successful exact-source CI, new-ELF execution/fork/keeper/web tests and
   affected CU/trace/size benches. Inspect actual failure outputs rather than
   weakening old identity checks or substituting LiteSVM's bundled token program.
2. Recheck source/artifact equivalence and current live dependency identities.
   Freeze the reviewed inputs for the chosen PC or VPS; the host choice remains
   pending. No old frozen directory is replaced or modified.
3. Complete and independently verify a fresh all-profile ≥24-hour campaign.
   Preserve old run evidence with its original source, fixtures and duration.
4. Update the current release record from actual results. Only then select the
   final mainnet-ready source tag; actual funded operator commissioning and the
   reviewed financial/signing packet remain separate requirements.

Static fixture/syntax checks are preparation evidence only. This record does not
claim that the candidate regressions or long campaign have executed successfully.
