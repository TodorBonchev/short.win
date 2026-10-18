# Independent fix review: draft 1.0.3

**Historical review, followed by a correction:** [AI3-FR01's subsequent fix](ai3-fr01-fix-1.0.3.md)
quarantines legacy empty carry before new deposits and introduces new program bytes.
This report's original candidate, artifact identities, outcomes and probes remain unchanged
evidence for the pre-guard revision; they do not certify the new release/fuzz gates.

Date: 2026-10-19. Outcome: **Q2/Q3/Q4 accepted; Q1 conditionally accepted**.
The new legacy-state migration finding **AI3-FR01 remains open**. This review is
independent AI code review and executable verification by reviewers separate from the
implementation author. It is not an external firm's audit, owner risk acceptance,
release sign-off, or verification of deployed markets.

## Revision and binary identity

| Item | Verified identity |
|---|---|
| Audited baseline | `b815269224ba0f5d946f49de7e113220c4067041` |
| Implementation | `60d26ad9541c7894a385118660a0ce75fd088367` |
| Exact-ref CI candidate | `9b910602ed076a6d4359c496960b3ffc0e7c6af8` |
| Fetched audit branch | `b4abcaa69e43a680b68fa5996d42fa7415a7c6fb` |
| Latest fetched `main` | `24f514c1` |
| Integrated source reviewed/tested | `708746eb0085016c37f4d73e2aea02031bd6efc9` |
| Exact-ref CI | [39144291500](https://github.com/TodorBonchev/short.win/actions/runs/39144291500), workflow_dispatch attempt 1, success; independently queried 25 successful jobs and one skipped scheduled-fuzz job |

All Rust source is identical between `9b910602` and `708746eb`:
`git diff 9b910602..708746eb -- '*.rs'` is empty. Latest-main integration adds crate
publishing metadata, attribution and documentation, and the IDL repository field; the
IDL remains version 1.0.3. That source comparison does not by itself establish identical
rebuilt program bytes or full CI on the integrated manifest set.

The reviewers downloaded `shortwin-program` and `verifiable-build` artifacts from the
exact-ref CI run rather than using the older preexisting local binaries. They independently
computed the following SHA-256 file hashes and compared the verifier release ELF with
the Anchor release ELF byte for byte (`cmp` succeeded):

| Artifact | SHA-256 |
|---|---|
| Release / verifier `shortwin.so` | `b88aa520c24301522b45871e73622a4d498603dc03a8f34ff8bbb557d7fa0647` |
| Test-venue `shortwin.so` | `994ad12c468f465937af21080ec3c48678bb1f197ab143ab44f91f98ca56132b` |
| Mock CPMM | `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9` |

The review also ran `anchor build --tools-version v1.57 --arch v3 --ignore-keys`
on the integrated program sources and manifests. It completed successfully. The locally
rebuilt release ELF and mock CPMM file hashes equal the table above, and `cmp` confirms
the release is byte-identical to the downloaded candidate. This supplies a canonical
Anchor rebuild across the publishing-metadata merge; it is not a new isolated verifier
run or full CI. The test-venue tests used the separately verified downloaded artifact.

The [hash record](../ci-1.0.3-hashes.txt) identifies the pinned verifier image and
toolchain. Comparing downloaded outputs verifies that recorded comparison; this review
did not independently run the verifier image. Native test binaries were rebuilt from the
review checkout, using prior targets as build caches, with explicit candidate ELF paths.

## Answers to the packet

| Question | Disposition | Review and independent evidence |
|---|---|---|
| Q1: AI3-F06 final-share carry | **Conditional accept.** The final-withdrawal correction follows the handler's ordering, pays whole-lamport carries, normalizes realized surplus into the pot and preserves allocated zero-share claims. It does not migrate an already-empty pre-fix pool. | [Accounting review](independent-accounting-1.0.3.md): seven carry tests, previous-source negative controls, verified-ELF signed withdrawal/new-cohort tests, vector replay and independent allocation/rollback/migration probes. AI3-FR01 must be resolved or its absence proved before complete upgrade acceptance. Historical pot and per-touch truncation remain separate. |
| Q2: AI3-F01–F05 keeper candidates | **Accept.** Candidate floor matches the program helper; searches continue across floor failures and preserve failure-chain behavior; creator placeholder 4 resolves across governed sets; raw LP ratios use fresh coherent triples, at most 99 keys per call. | [Keeper review](independent-keeper-1.0.3.md): actual on-chain verification and planning paths compared; 98 keeper unit tests and 15 pool-selection LiteSVM scenarios passed, including pool-34 retry and both failing-group positions. No additional defect identified in this scope. |
| Q3: AI3-L03/L04 readers and recovery | **Accept.** Supported typed versions remain readable; unknown versions do not enter typed consumers. A ranking batch is dequeued/published only after every RPC group succeeds. | [Reader/recovery review](independent-readers-1.0.3.md): four typed-version cases, four batching cases and independent unsupported-guard/recovery probe passed; the keeper scenarios verify complete retry without rediscovery. Failed crank isolation still reaches a real ConstraintSeeds failure on the verified program. |
| Q4: AI3-L01 ALT metadata | **Accept.** A truncated metadata header cannot be an empty ALT; a full empty table and complete address bodies parse. | [Reader review](independent-readers-1.0.3.md): three common-library cases and an exhaustive length/roundtrip probe passed. Extracting the exact baseline parser and switching the same probe to it fails at a four-byte initialized header, as expected. |

## New finding and retained boundaries

**AI3-FR01: legacy empty-pool carry survives upgrade.** The accounting reviewer
produced an empty carry-bearing state through the unmodified baseline's public API and
compared it to the input of a candidate public-API upgrade probe. After required sweeping,
a new one-token cohort receives 1,001 lamports despite contributing only one new lamport;
the historical 1,000 lamports came from the previous cohort. Candidate `withdraw_in`
resets carries only when a withdrawal reaches zero shares. Candidate `deposit_in` does
not repair a state that was already empty before that code ran. No affected deployed
market is claimed. Required treatment is inventory evidence excluding the legacy state,
or an ownership-preserving migration with regression and fix-batch gates; blindly clearing
carry is not an approved allocation policy. The [tracker](../findings.md#tracker) and
[known-issues §8](../known-issues.md#8-open-items-from-the-103-fix-review) keep it open.

Final-holder allocation also differs from historical pro-rata allocation: a tiny late holder
can receive still-unallocated carry after the earlier large holder leaves. That is the
documented policy retained by this correction, not a reconstructed old-owner entitlement.
The accounting reviewer also reran the pot-race characterization and quantified repeated
per-lender truncation; global carry reset does not solve either mechanism.

The [written unresolved treatment](unresolved-1.0.3.md) covers all six packet-excluded
prior findings, all 20 unimplemented AI3 leads, existing duplicate boundaries and the new
migration condition. Each item has an evidence limit, rationale and completion requirement.
No open item is silently rejected, acknowledged or absorbed into a broader old acceptance.
The immutable 90-file original audit archive still passes its SHA256SUMS check.

## Integration correction and release limits

Across the three independent reviewers, **157 test cases passed**: 113 keeper cases,
13 reader/recovery cases and 31 accounting/baseline-witness cases, including harness
checksum tests. The old carry implementation fails six new behavioral regressions;
the old ALT parser fails the independent size-matrix assertion. The specialist reports
give the exact commands, filtered/zero-test exclusions and reproducible probe source.

Integrating the latest published-crate work initially caused
`python3 scripts/audit_scope.py check` to reject four unclassified files:
the math LICENSE-APACHE/NOTICE and the venue-state/program READMEs. The review explicitly
classifies them as unbuilt license/attribution or contextual documentation and regenerates
`docs/audit/scope.txt`. The check then passes; no production unit is removed or widened.
This review introduces no runtime program or keeper change.

Implemented tracker rows remain **in progress**. The 24-hour fuzz run must identify the
final program/fuzzer bytes and finish with zero failures; the historical waiver covers
AIR-01–AIR-24 only. Existing open economic/venue issues and AI3-FR01 require resolution or
the exact explicit owner disposition permitted by policy. The independently verified old
CI run is evidence for `9b910602`, not an assertion that a newer merge has completed CI.
This work does not supply an external firm's acceptance, tag a release or deploy a program.
