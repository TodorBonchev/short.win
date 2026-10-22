# 1.0.3 integration and devnet readiness, 2026-10-22

**Keep the version at unreleased 1.0.3. Stable tagging and full public-devnet
market testing are not ready.** No 1.0.3 release tag exists; further preparation
does not require calling the same unreleased candidate 1.0.4. A version change
would also change the program's embedded `source_release` and artifact identity.

This integration uses branch `release/1.0.3-devnet-prep-2026-10-22`, starting at
merge `b31c02a091008da18c39d7836c8e0e9d6d018973`. It combines:

- `audit-1.0.3-gates` at `30ba1f54b09bb0fae5a01072c1b5750e8251b76d`, including
  the completed prior fuzz and separately attributed CI evidence;
- the saved audit checkpoint and article review at
  `f492ec2dbf161ee4034caa1343ef64813a6e2854`.

The original checkout and other audit worktrees are preserved. The corrected
publication text is
[`WHO_WINS_WHEN_YOU_LET_PEOPLE_SHORT_MEMECOINS_CORRECTED_2026-10-22.md`](../articles/WHO_WINS_WHEN_YOU_LET_PEOPLE_SHORT_MEMECOINS_CORRECTED_2026-10-22.md).
Publication remains a manual owner action.

## Changes and current checks

The article review corrects the specification and `add_margin` module comment:
positions lock rent-curve coefficients/minimum terms, while current utilization
and its EMA affect ongoing rent. It does not change protocol economics, account
layouts, adapters or governance limits. Its
[engineering review](../design/MEMECOIN_SHORT_ARTICLE_REVIEW_2026-10-22.md)
explains which proposed changes require more evidence.

The additional web fix makes default transaction Explorer links select devnet
when `VITE_CLUSTER=devnet`; explicit templates and existing other-cluster defaults
are preserved. Existing configuration tests cover the change.

Fresh checks against this integrated candidate include the locked SBF build,
unchanged IDL/client generation,
427 math/WASM-facade/Squads host tests at `PROPTEST_CASES=2000`, Rust formatting,
the static 344-entry fixture check, scope check, frozen JS dependency installation,
client typecheck and 30 tests, six focused web files with 24 passing tests,
the keeper event-origin regression, scoped web lint, fresh WASM/facade smoke and
the web production build including typechecking. The retained check record
distinguishes fresh results from older full CI. The web build used explicit
localnet compile settings: it is not a configured running devnet service.

[Retained commands, logs, public RPC observations and checksums](probes/release-wrap-2026-10-22/README.md)
make these results reviewable without relying on the local build cache.

## Artifact identity and release gates

| Artifact | File SHA-256 | Bytes | Qualification |
|---|---|---:|---|
| Previous canonical release | `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc` | 1644024 | Prior CI/verifier and completed ≥24-hour fuzz, as separately attributed in the earlier release record |
| Fresh integrated candidate | `d8d08784b3b3081d5fe1cf7aaff319e7e9e063733c308bd2e9e7ed8981bee182` | 1644024 | Fresh local release build; new final-byte qualification remains required |

The files differ at exactly one byte: offset 330428 in `.text`, changing an
Anchor error-location immediate from line 72 to 73 after the corrected module
comment. The instruction concerns `ZeroAmount`; `.rodata` and every other byte
match. This diagnostic difference explains the new hash. It does **not** make
the old final-byte fuzz record a run on the new artifact.

The fresh build used Agave 4.1.2, platform-tools v1.57 and SBPF v3. The installed
Anchor reports 1.2.0, but its local AVM source receipt differs from CI's pinned
Anchor revision. Until an exact pinned/verifiable build reproduces this candidate,
do not label this local output a newly verified release binary. IDL equality,
A49 release checks and `security.txt` checks passed independently of that limit.
The attempted official verifier setup is concretely blocked: this process lacks
Docker socket access, and a bounded noninteractive `sudo -n` probe required a
password. No privileged operation or verifier build ran, and the image cache's
availability remains unknown. See the
[retained verifier record](probes/release-wrap-2026-10-22/verifier/readiness.md).
The operator must provide an accessible approved build environment before that
gate can pass; no system permissions were changed by this preparation.

Before a stable `v1.0.3` tag, finish the applicable exact-source/full build gates
and ≥24-hour fuzz on the final selected program/fuzzer/fixture inputs under
[AUDIT §12](../AUDIT.md#12-logistics-and-change-policy). Earlier durations are not
added to a new run. Preserve the existing failed mainnet Jupiter live-pin check;
it requires an explicit upstream/configuration disposition before a mainnet
release, rather than an unexplained pin update.

## Devnet and operations

The dedicated devnet payer/proposed upgrade authority is
`3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14`. Its private key is stored outside
Git at `/home/todor_dev/.config/shortwin/devnet/release-testing-2026-10-22.json`,
with directory mode 0700 and key mode 0600. No seed or private key is included in
this record. The first two-SOL faucet request failed; the initial balance was zero.
The existing program signer has the canonical program address
`5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK`; it is distinct from the payer.

Actual public-devnet observations found the short.win program/config absent,
different Raydium devnet program IDs, mutable devnet Squads where supported
governance tooling requires immutability, and different pump deployment slots
and allocations from the retained mainnet fixture profile. RPC selection alone
does not establish adapter or governance compatibility. A sampled Pyth update
was stale for the compiled maximum age; freshness must pass when testing occurs.
The [devnet readiness record](../deployment/DEVNET_1_0_3_READINESS_2026_10_22.md)
retains concrete public addresses, slots, size-based rent estimates and the
separate remaining test/deployment requirements. The planning funding target is
20 devnet SOL, allowing for a concurrent upload buffer, same-size ProgramData
allocation, initialization and margin; it is not a measured fee quote.

A program-only deployment can verify loader address, ELF hash and upgrade
authority, but cannot demonstrate lending, borrowing, settlement or keeper
operation. Initialization requires the current program upgrade authority's
signature; public callers cannot initialize the config merely because a program
has been deployed. Do not remove that authority before initialization. Full
testing needs a reviewed target-cluster profile and venue/state compatibility;
do not weaken governance checks, substitute canonical program IDs or deploy the
`test-venue` artifact to obtain a green smoke test.

Before market tests, stage compatible 304-byte VenueGuard clients/indexer and
destination-3 decoding, fund and identify a non-dry-run keeper, specify covered
markets and top-up/alert ownership, and perform the required hashed revalidation
of growable venue guard entries before admitting opens. Existing local-validator
E2E scripts use fixture programs and synthetic wallets; they must not be pointed
at devnet.

## Audit boundary

The [saved whole-repo audit checkpoint](2026-10-22-progress/README.md) remains
incomplete: 36 hypotheses, 13 independent decisions (six rejected, seven requiring
validation), 23 without independent disposition, and no confirmed new finding.
The source coverage ledger is 73 covered, 42 candidate and eight blocked units.
The two provider-stopped tasks remain excluded and have not been reassigned or
retried. These are review gaps and unresolved hypotheses, not 36 confirmed bugs.
The 19 earlier technical tracker closures and earlier AI-assisted fix-review
route do not close the new checkpoint or establish whole-codebase safety.

Tagging, deployment, initialized market testing and stable/mainnet release must
be reported separately. This preparation record is not a tag, deployment receipt
or security approval.
