# short.win pooled v1.1.1 software release candidate

The owner authorized preparation of a new **v1.1.1 software tag** after the
v1.1.0 source audit and remediation work. This record describes the candidate
state on 1 November 2026. It does not authorize deployment, initialization,
governance or financial transactions, funded operation, or mainnet activation.

`v1.1.0` is immutable. Never move or replace it. The release build input below
passed the pinned `simple-pooled` workflow. This documentation-only completion
commit changes release evidence only. The annotated `v1.1.1` tag identifies the
final tag commit and tree in tag metadata, avoiding a self-referential commit field
inside the tagged file. The tagged tree must keep the qualified program inputs and
reproduce the same ELF and IDL hashes. If a later check finds a defect, keep
`v1.1.1` immutable and prepare a new `v1.1.2` fix release.

## Candidate software identity

| Item | Candidate value |
| --- | --- |
| Program ID | `BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh` |
| Release build input commit | `61251bf427e6a805e2a9f9b207cbd0449026fad2` |
| Release build input tree | `6286ba678c70304e683afdf468aecba2ac5495c5` |
| Program ELF SHA-256 | `86147af04bda2c1fbab92ad670dc8d7cc272162724787dac666344bf24b547f2` |
| Program ELF size | `787080` bytes |
| Generated IDL SHA-256 | `b60f327736affd6d2b2904c6fc7487c3af258704cffebeb97e8b62e91bf541d5` |
| Generated IDL size | `101361` bytes |
| SBF build log SHA-256 | `c55158e99a57e374f5b8a248ab95a700bce9f004aa860ead3627258279e4cab8` |
| Exact-commit CI | [`simple-pooled` run 41288472696](https://github.com/TodorBonchev/short.win/actions/runs/41288472696) |
| CI artifacts | validation `13076083053`; fuzz-smoke `13076451575`; route-suite `13076202511` |
| Audit baseline | `1fbd13c70ffc754563ec7647aa65aba17b9b5c4c` (`v1.1.0`) |
| Audit report SHA-256 | `57db6d86ffb09283373d8a5001fa92f7f57743eae17ab1002d783898d7a34095` |
| Structured findings SHA-256 | `69bb52c62732ee1f066a1f135e7d19313e7bc0234a3d2af04f168fbe740d9ede` |
| Runtime evidence SHA-256 | `6b898c1569007f6b1a20a1e0d017cc1a904110d512f6a0042e1b73dbcb379e47` |

The audit and runtime files named by those hashes are retained in the external
`security-audit-skill/short.win/run-3` evidence bundle. They are not silently
copied into this repository or treated as build output.

## Source audit and remediation

The full-repository source audit reviewed immutable `v1.1.0` in five Rust-auditor
passes and a coverage-led independent review:

- 159 coverage units reached a clean final coverage critic.
- 32 candidate fingerprints received independent dispositions.
- 31 retained confirmed or needs-validation records received a second, fresh
  record review.
- 12 findings were confirmed: 10 medium and 2 low.
- 19 source-grounded records still need owner, runtime, provider, deployment, or
  operational validation; they do not have severity.
- 1 candidate was rejected.
- All 12 confirmed root causes have patches, divided into five independently
  approved patch groups.

The changes cover transaction-outcome preservation, PumpSwap risk-fee accounting,
Token-2022 native-mint admission, gapcal signature and cache confinement, image
metadata path confinement, and indexer finality, ordering, lifecycle, history and
rollup consistency. Migration `services/indexer/migrations/0104_projection_ordering.sql`
is part of this candidate and must ship with the indexer changes.

Release hygiene also reorders two adjacent `services/keeper` module declarations
to the deterministic order required by the pinned Rust formatter. That one-line
mechanical change has no runtime semantics and makes the full services formatting
gate reproducible.

The **AGENTIC REVIEWED** mark means coverage-led source review, independent
candidate and final-record verification, and independent source review of every
patch. It is not an external audit-firm opinion and does not claim complete runtime
qualification. No executable demonstration, live RPC request, key use, signed
transaction, cluster action or deployed probe was used for the audit.

## Completed post-fix checks

The external runtime matrix records these exact completed results against
hash-bound remediation fixtures in independently reviewed offline sandboxes:

- **218 Rust tests passed**: 142 root-workspace library tests, 1 targeted
  `shortwin-simple` native-mint regression and 75 indexer native tests.
- The final gapcal suite passed **55/55** after correcting only obsolete synthetic
  signature fixtures; production validation remained strict.
- Syntax checks passed for **8 patched TypeScript files**.
- Python compilation checks passed for **3 patched Python files**.
- The final SBF attempt completed release compilation for the legacy program, but
  its existing program-loaded LiteSVM test still did not find the expected
  `target/deploy/shortwin.so`; no patched program was loaded in that test.

These local checks are partial successful evidence, not a claim that every release
surface ran. No executed Rust source assertion failed in the retained results.

The later exact-commit GitHub Actions run for
`61251bf427e6a805e2a9f9b207cbd0449026fad2` completed successfully and provides
the release qualification for the active pooled program:

- Native format, metadata and source checks passed.
- The pinned SBF v1.57/v3 toolchain built the program, and the stack diagnostic
  gate passed.
- The generated Anchor IDL matched the committed IDL byte for byte.
- **71 Rust tests passed**, including the active program's existing native tests.
- Actual-SBF runtime and ledger tests passed with the built program loaded.
- The randomized model runner completed **200000 cases** and the program runner
  completed **32 cases**.
- The current-route compatibility suite passed.
- Client tests passed **39/39**, web tests passed **10/10**, and the production web
  build completed successfully.

The CI run retained the validation, fuzz-smoke and route-suite artifacts identified
in the software-identity table. The recorded ELF, IDL and SBF-log hashes come from
that exact run, not from the earlier partial local matrix.

## Disclosed checks outside the pooled CI qualification

The following repository or operational checks remain separate and must not be
represented as passing:

1. Supply an offline PostgreSQL distribution/archive to the embedded-PostgreSQL
   harness and run all four analytics integration assertions. The existing offline
   attempt compiled the tests but stopped while trying to bootstrap PostgreSQL.
2. The earlier local simple-client attempt lacked an offline dependency closure.
   The exact-commit GitHub Actions run subsequently completed the client and web
   tests and production build, but that does not turn the earlier local attempt
   into a pass.
3. Record the documentation-only completion commit and tree, the qualified build
   input commit and tree, the CI URL, and the artifact hashes in the annotated
   `v1.1.1` tag. Verify before tagging that no program input changed after the
   qualified build-input commit.

The release operator must also recheck every `needs_validation` item that depends
on actual provider identity, loader/ProgramData, cluster genesis, operating policy,
funding, database migration/backup, alert delivery, or deployed configuration.

## Version and evidence boundaries

Active pooled program, helper, runtime-test, client and web metadata are 1.1.1.
The legacy `programs/shortwin` package remains on its historical 1.0.3 line; this
repository-wide remediation does not relabel that historical program as 1.1.1.
Historical v1.1.0 ELF/IDL hashes, captured routes, CI and the 24-hour financial
campaign remain valid evidence for their frozen bytes only. They do not qualify
changed v1.1.1 source, services, policy, fixtures or generated artifacts.

After the final commit and `v1.1.1` tag are pushed, run the separately authorized
24-hour offline fuzz campaign against an exact detached checkout of that tag. The
campaign may use only existing maintained targets and must bind its configuration,
corpus identity, start/end times, exit status and result hashes to the tag. A
failure stops the affected target at its minimum retained diagnostic and opens a
new source-fix review; it does not justify moving the tag.

## Commissioning remains separate

Before any deployment, independently verify private key custody, public role
assignments, upgrade authority, cluster genesis, canonical Jupiter loader and
ProgramData identities, service/database artifacts, operator funding and limits,
provider capacity, backup/restore, alerts and exact production configuration.
Changed upstream bytes require renewed compatibility review. Tagging software does
not send a transaction or authorize activation.
