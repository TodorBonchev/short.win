# 1.0.3 integration and devnet readiness, 2026-10-22

**The program-only devnet deployment succeeded. Keep the version at unreleased
1.0.3; stable tagging and full public-devnet market testing are not ready.**
No 1.0.3 release tag exists; further preparation
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
and the token-price EMA affect ongoing rent. It does not change protocol economics, account
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
| Superseded integrated build, before line-count correction | `d8d08784b3b3081d5fe1cf7aaff319e7e9e063733c308bd2e9e7ed8981bee182` | 1644024 | Historical local build; not selected for deployment |
| Selected `7b11cc7` release, host and pinned Docker | `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc` | 1644024 | Both fresh builds exactly reproduce the canonical artifact |

The canonical and superseded intermediate files differ at exactly one byte: offset 330428 in `.text`, changing an
Anchor error-location immediate from line 72 to 73 after the corrected module
comment. The instruction concerns `ZeroAmount`; `.rodata` and every other byte
match. This diagnostic difference explains the new hash. It does **not** make
the old final-byte fuzz record a run on the new artifact.

The superseded local build used Agave 4.1.2, platform-tools v1.57 and SBPF v3. The installed
Anchor reports 1.2.0, but its local AVM source receipt differs from CI's pinned
Anchor revision. Its local version string alone was not pinned-build proof.
IDL equality, A49 release checks and `security.txt` checks passed independently
of that limit. The new Docker reproduction below resolves executable provenance
for the selected canonical artifact, without changing that AVM source receipt.
The first official verifier setup failed because this process lacked Docker
socket access; its [dated record](probes/release-wrap-2026-10-22/verifier/readiness.md)
is retained. **Docker access was subsequently repaired under owner authorization:**
the account now belongs to the Docker group, `docker.service` is active, and a
`newgrp docker` child reaches Docker Engine 29.9.1. Existing login processes need
a group refresh or new login. Socket mode and boot enablement were not changed.
The [access receipt](probes/release-wrap-2026-10-22/verifier/docker-access.json)
closes the environment blocker, separately from the build result.

Commit `7b11cc7f85503474c6b87e0ee8b2fedd51bbc249` preserves the corrected comment's
meaning using the original line count. Its native release build and real Docker
verifier **both passed and exactly reproduce the earlier canonical bytes**.
The normal test-venue and mock builds also reproduce their canonical artifacts.
The release trimmed hash is
`f3d8dd7a5c62385abc6306e2da395ff666a1f69234ea50f2e7eb0523ab686250`.
The verifier used solana-verify 0.5.2, image digest
`sha256:2e0b78f44ee76612e9260c7c988570c5e14de6fbd93e0ab07115ec7054473b4f`,
platform-tools v1.57 and SBPF v3. See the
[build and source-equivalence evidence](probes/release-wrap-2026-10-22/byte-rebuild/SUMMARY.md).

Fuzzer/harness, fixtures, runtime math and locked dependency inputs are unchanged
from the completed run's frozen source. The program source delta is the corrected
comment; actual rebuilt release/test artifacts establish byte identity. Therefore
the owner's completed ≥24-hour record applies to these same runtime inputs,
with its original source, runner, dates, profile and coverage limits retained.
No new fuzz run was performed or credited. The later services/web changes retain
their separate CI and focused checks; fuzz does not validate those services.

Before a stable `v1.0.3` tag, finish its remaining release/operating requirements
under [AUDIT §12](../AUDIT.md#12-logistics-and-change-policy). The canonical
program/fuzzer/fixture fuzz requirement is satisfied by the completed original
run and demonstrated equivalence, with no combined durations. Preserve the
existing failed mainnet Jupiter live-pin check;
it requires an explicit upstream/configuration disposition before a mainnet
release, rather than an unexplained pin update.

### Fresh qualification interrupted

The fresh existing-harness run started at **2026-10-22 01:56:30 UTC** against
source `bf166672efee578b6a2a1835e0dd53729c548671`. Its program worker then returned
an actual provider safety block. The additive launch review was interrupted,
and the verified supervisor/runner process group was terminated at
**01:59:09 UTC**; both processes were confirmed absent. The task has not been
rephrased, restarted or reassigned. This is an interrupted run, with **no
qualifying 24-hour result** and no prior-duration credit. See the
[root interruption record](probes/release-wrap-2026-10-22/fuzz-provider-stop.json).
The preparation checks retained above predate this stop. This interrupted attempt
remains unqualified and stopped. The later normal build comparison independently
returns to the already qualified canonical artifact; it does not rehabilitate
the interrupted run or establish a complete security audit.

## Devnet and operations

The owner delegated project choices. The [selected devnet decisions](../deployment/devnet-decisions-2026-10-22.md)
close those policy questions: first deploy only the uninitialized program, retain
upgradeability under the devnet payer, keep version 1.0.3, cap the deployment
budget at 20 SOL and preserve at least 5 SOL. No tag is created in this phase.
The [local operator plan](../deployment/devnet-operator-plan-2026-10-22.md)
records selected service settings for later initialized/market testing; those
services are not prerequisites to the loader-only rehearsal.

The [concrete deployment packet](probes/release-wrap-2026-10-22/deployment-packet/deployment-packet.md)
bound the program identity, external upload-buffer signer and successful unsigned
buffer-preparation simulation. Its estimated net debit was 8.361938880 devnet
SOL; the conservative envelope was 16.714459640 SOL, below the selected 20-SOL
ceiling. The CLI has no aggregate spending-cap option. All generic policy
choices were selected before either signed attempt.

**Current deployment result:** the owner separately approved a same-buffer
resume. It finalized successfully in transaction
[`2twpM5sUnQeAJLVTeKo9gHeDPfpU3iNitPZNg9UVm7At6zMyQCn6DATh9azQRk9rd1DzCi315W8HKZLbDpGhVm4j`](https://explorer.solana.com/tx/2twpM5sUnQeAJLVTeKo9gHeDPfpU3iNitPZNg9UVm7At6zMyQCn6DATh9azQRk9rd1DzCi315W8HKZLbDpGhVm4j?cluster=devnet)
at slot **513919880**. Program
`5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK` is executable; its
ProgramData contains the exact 1,644,024-byte canonical ELF (full SHA-256
`d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc`).
The temporary upgrade authority is payer
`3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14`; the upload buffer was
consumed and the payer held **16.638061120 SOL** after deployment. GlobalConfig,
VenueGuard and OracleCache remain absent: this is an **uninitialized,
program-only** deployment. The total debit from the initial 25 SOL was
8.361938880 SOL. See the [deployment receipt](../deployment/DEVNET_1_0_3_DEPLOYMENT_2026_10_22.md)
and [retained transaction evidence](probes/release-wrap-2026-10-22/deployment-packet/README.md).

**Historical first attempt:** the first approved devnet CLI invocation failed
while writing the program, reporting `Max retries exceeded`. At that time, the
approved buffer held 8.352520760 SOL and 14 matching upload chunks; the payer
held 16.647399240 SOL, and the program/ProgramData were absent. Fifteen
transactions were confirmed successful; 80,000 lamports in fees were spent.
The [failure receipts](probes/release-wrap-2026-10-22/deployment-packet/failed-attempt-retained-files.json)
record its bounded result. At that point, the original one-attempt
authorization had been spent; the later resume required and received separate
owner approval.

The [historical read-only recovery proposal](probes/release-wrap-2026-10-22/deployment-packet/recovery-packet.md)
retained the existing buffer and verified artifact. The first missing write
simulated successfully. Agave's resume path would send only the 1,699 remaining
chunks; one successful pass was estimated to debit 0.009338120 SOL. The proposed route
tried the CLI's default QUIC TPU route with five signing cycles, subject to a
fresh 0.05-SOL additional operator ceiling and separate explicit transaction
approval. That approval was given and the resume succeeded; this does not prove
which network route sent each write or the cause of the first failure.

The dedicated devnet payer and actual temporary upgrade authority is
`3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14`. Its private key is stored outside
Git at `/home/todor_dev/.config/shortwin/devnet/release-testing-2026-10-22.json`,
with directory mode 0700 and key mode 0600. No seed or private key is included in
this record. The first two-SOL faucet request failed; the initial balance was zero.
**Funding update, 07:20:41 UTC:** a fresh confirmed devnet RPC check at slot
513,807,141 shows **25 SOL**, exceeding the 20-SOL planning target. Funding is
complete for the planned reserve. This funding observation predates deployment;
the current post-deployment balance is recorded above.
[Public funding observation](probes/release-wrap-2026-10-22/devnet/funding-2026-10-22.json).
The existing program signer has the canonical program address
`5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK`; it is distinct from the payer.

Initial public-devnet observations found the short.win program/config absent,
different Raydium devnet program IDs, mutable devnet Squads where supported
governance tooling requires immutability, and different pump deployment slots
and allocations from the retained mainnet fixture profile. RPC selection alone
does not establish adapter or governance compatibility. A sampled Pyth update
was stale for the compiled maximum age; freshness must pass when testing occurs.
The [devnet readiness record](../deployment/DEVNET_1_0_3_READINESS_2026_10_22.md)
retains concrete public addresses, slots, size-based rent estimates, the
completed loader-only deployment and remaining market requirements. The
20-devnet-SOL planning target was an upload reserve, not a measured fee quote.

A program-only deployment verified loader address, ELF hash and upgrade
authority, but did not demonstrate lending, borrowing, settlement or keeper
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

Tagging, program deployment, initialized market testing and stable/mainnet release
are separate milestones. This record links the program-only deployment receipt;
it is not a stable tag, market-test result or security approval.
