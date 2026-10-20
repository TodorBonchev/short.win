# Independent AI integration review addendum for draft 1.0.3

Date: 2026-10-21 local. Hash verification checkpoint: 2026-10-20 22:25:52 UTC.
Exact candidate: **`e04a6776f1c2f5e02e581d76dd1255c416270275`**.

**Outcome: accept the focused integration described below, conditional on successful
exact-ref full CI, the final-profile ≥24-hour fuzz gate and operational preflight.**
No additional blocking source defect was identified in this scope. This is not
release approval or blanket closure of findings.

## Review provenance and scope

This is a Codex agent-authored AI-assisted review, not an external security firm's
report, accreditation or professional signature. It follows the batch-specific route
authorized in [the owner decisions](owner-decisions-1.0.3-2026-10-21.md) and supplements
[the complete-digest fix review](independent-ai-fix-review-1.0.3-2026-10-21.md), pinned
to `f5a8ccefa92755661964e63d41c458fb1072e9d5`. That earlier report retains its own
authorship disclosures and evidence limits.

Reviewer `keeper_fix_review` independently inspected the TLS changes, replay record
shape, captured event characterization, new fixture pins, baseline/profile integration
and the corresponding safeguards. It did **author** the legacy keeper cleanup
(`604154c3`) and current-schema upgrade test (`ec25193d`). Agent
`remote_scope_check`, which did not author the keeper cleanup, independently reviewed
that production diff and tests and found no blocking issue. The coordinating root
agent independently reviewed the upgrade-test diff and found no blocking issue.
Those peer conclusions are source reviews; neither is presented as an independent
rerun. Mutual AI peer review is not organizational independence from an external firm.

The inspected candidate includes the following focused changes:

| Commit | Scope and disposition |
|---|---|
| `b54c43842bae5f1a448afd1e8ad7258369f6e577` | **Accept:** patched services TLS dependencies and explicit Mozilla-root WebSocket connector preserve certificate/hostname validation and redacted errors. |
| `1cee0c16bc1365ced50463ab9dc43c32d82659bb` | **Accept:** three replay recordings regenerated for the 304-byte guard. The commit changes recordings only; no compatibility padding or weakened consistency assertion was introduced. |
| `604154c36bb13ae850134bc0a93b809f5f1ea1a4` | **Accept with disclosed authorship and separate peer review:** retire the undeployed 1.0.0 V2 keeper path while preserving current extras retries, exit redirection and the observed-deploy simulation floor. |
| `82b3eb06bef2794b5c7ae3ea5cf1e159bab0598d` | **Accept:** captured event fixture and regression characterize the existing indexer decoder. This commit adds no production decoding change. |
| `ec25193d1ea3ea480d6c4beb93cd5171fe94a919` | **Accept with disclosed test authorship and root source review:** current 304-byte same-schema loader-upgrade regression; historical 224-byte position-exit case remains explicitly scoped. |
| `e04a6776f1c2f5e02e581d76dd1255c416270275` | **Accept within supported SOL/WSOL fixture paths:** additive venue pins, selected baseline, browser genesis/vectors and compatibility characterization. No new quote asset is admitted. |

Concurrent landscape/phone promotional and narration work is retained in the ancestry.
It was not security-audited by this addendum. Earlier policy/UI work and other
AIR2/AI3 leads keep their separately recorded scope and dispositions.

## Source conclusions

**TLS.** Services resolve `rustls-webpki` 0.103.16 and
`webpki-roots`/`webpki-root-certs` 1.0.10. The primary advisories identify these patched
versions: [RUSTSEC-2026-0382](https://raw.githubusercontent.com/rustsec/advisory-db/main/crates/rustls-webpki/RUSTSEC-2026-0382.md),
[RUSTSEC-2026-0381](https://raw.githubusercontent.com/rustsec/advisory-db/main/crates/webpki-roots/RUSTSEC-2026-0381.md)
and [RUSTSEC-2026-0383](https://raw.githubusercontent.com/rustsec/advisory-db/main/crates/webpki-root-certs/RUSTSEC-2026-0383.md).
This reviewer independently read those entries and the local upstream
`tokio-tungstenite` 0.29 connector implementation. The explicit connector uses the
patched Mozilla anchors and normal Rustls verification; the WebSocket domain still
becomes the verified `ServerName`. The ordinary `ws` path and redaction remain.
No dangerous verifier, OS-root policy expansion or advisory exception was introduced.
The documented `__rustls-tls` feature is an upstream maintenance dependency to recheck
on dependency upgrades.

**Replay and event evidence.** Static parsing in this pass found a 304-byte
VenueGuard in each of the three regenerated recordings. The captured pump event
regression requires the observed 390-byte event to preserve the entire previous
382-byte prefix and append eight zero bytes, compares the known SOL row and refuses
a synthetic non-SOL quote and truncated prefix. The tail's meaning remains unknown.
This is signed local characterization on hashed upstream ELFs, not a mainnet
transaction or proof of every future event layout.

**Keeper cleanup.** The retired builder served short.win 1.0.0's 20-key parser.
Never-deployed status removes that operational migration requirement. Current
curve exits use the 10-key v3 list; current/previous/stable governed VenueExtras
choices and retry predicates remain. `UseSolSettle`/`UseExpire` still permit only
the guarded follow-up. Current parse failures and unexpected `CurveStraddle` are
ordinary `ExitRefused` alerts. A curve simulation from before the independently
observed short.win deployment is held and retried. Removing format negotiation
does not remove that bank floor.

**Upgrade coverage.** The retained 1.0.0 case passed, but its 224-byte guard is
unchanged and cannot be decoded as the current guard by native hooks. It is
historical position-exit evidence, not guard migration or new-open readiness.
The new current-schema case uses real loader instructions, preserves the 304-byte
guard, all four full digest records, configuration and live financial accounts,
holds new opens while a real venue-upgrade latch remains set, allows an old close,
rejects a mismatched pump review slot without writing the guard, then revalidates
the exact slot and admits a fresh open. The venue's mainnet authority signature is
forged only through the existing test-only helper; short.win's upgrade uses its
real test authority signature. Three unrelated extensible venues use ordinary
harness ProgramData mocks to exercise their digest storage slots, not their adapters.

**New upstream fixtures.** The additive manifest retains all prior account entries
and snapshots. Both new ProgramData accounts authenticate the expected program
mapping, loader owner, 10 MiB allocation, deploy slot and unchanged authority.
This reviewer independently reconstructed whole-account hashes from header, saved
ELF and zero tail, and checked the complete trimmed hashes:

| Venue | Deploy slot | Trimmed ELF SHA-256 |
|---|---|---|
| pump | 458,921,440 | `756c04ab1ac76122dcdcf2eeb0073bb139087b23d58d3f0d6c895510ab59d582` |
| PumpSwap | 458,921,517 | `4d5fc41918374b24b16d8372d14a1ac64c768c83a2971bfeef1c8660dad649e7` |

The fixture-group date is the local observation date; the retained block-time
responses date the actual deployments to October 19. The published October 13 IDL
is an interface reference, not a verified source build for these later ELFs.
`PumpDeploy::CURRENT`, the fixture guard slots and the new named constant agree.
Historical deploy variants retain their own slots instead of inheriting CURRENT.

Full hashed fixture bytes establish identity. The instruction/depth probes establish
a behavioral profile shared with `Oct2026Chain`; the explicit `probe_profile`
mapping avoids claiming those probes distinguish the new ELF. The `held` helper's
64-byte ELF-header classification is test instrumentation, not full-byte authentication.
The new curve differential permits changed CU and error-origin lines while requiring
the complete old event prefix, exactly the observed eight-zero-byte tail and equal
named account state. It does not silently drop all event evidence.

Supported curves remain SOL-quoted at depth zero; PumpSwap remains WSOL with the
ordinary SPL Token quote path. Signed execution on the new ELFs supplies scoped
compatibility evidence for their readonly WSOL mint path. Historical state snapshots
are retained; the labeled post-deploy-clock control does not turn them into a coherent
new mainnet bank snapshot. No full upstream implementation audit is claimed.

Pump and PumpSwap remain **exact-slot latched** at the loader cap. They are not among
the four extensible venues with stored full-digest extension checks. A supplied hash
does not permit their unexpected-slot revalidation. Their new off-chain full hashes
must not be described as on-chain stored digest protection.

## Executed checks and attribution

Tests below were executed during integration before this final documentation pass,
using explicit frozen SBF paths. They are not claimed as a new clean-checkout rerun
after the candidate commit. This pass independently verified final source/artifact
identity and did not rerun the test suites.

| Executor | Evidence and actual result |
|---|---|
| This reviewer, on promoted `Oct2026QuoteFees` CURRENT | `curve_exits::`: **2 passed**, 151 filtered, 24.73 s; two stale-bank simulations held then recovered, persistent current refusals paged then recovered. |
| This reviewer, on promoted CURRENT | `graduation::`: **12 passed**, 141 filtered, 24.54 s; includes synthetic liquidation/expiry, third-party completion race, S3 and the PumpSwap warm-up continuation. |
| This reviewer, cleanup implementation checks | Keeper library **95 passed**; extras scenarios **2 passed**; strict all-target keeper/keeper-LiteSVM Clippy, formatting and diff checks passed. These implementation executions do not supply independent source authorship. |
| This reviewer, upgrade test author | Final `upgrade_it` binary: **2 passed**, zero filtered, 20.20 s; scoped strict Clippy passed. A first new-case setup attempt lacked unrelated ProgramData mocks; that fixture error was corrected before the reported pass. |
| `remote_scope_check` | Reported TLS regressions **2**, common library **10** and ops library **17** passed; strict all-target indexer Clippy, formatting, deny/audit checks passed. This reviewer independently inspected source/dependency behavior and did not rerun those suites. |
| `reader_fix_review` | Reported strict committed replay-recording consistency passed; new curve differential compared **180 transactions**, modern-clock SOL trades passed, and renewed PumpSwap T-PFEE caller paths passed on two pools. Those executions are attributed to the reader agent. |
| Coordinating root | Reported browser vector regeneration from signed execution and canonical Anchor/test-venue rebuilds with unchanged SBF hashes. This reviewer verified hashes, not the build processes. |

There is no aggregate unique-test total: profiles, suites and focused reruns overlap.
The relevant independently executed promoted-CURRENT commands were:

```sh
export CARGO_BUILD_JOBS=2
export CARGO_TARGET_DIR=/root/short.win/services/target
export SHORTWIN_SO=/root/fuzz/fullsha-103-20261021-a130fd98/bin/shortwin-release.so
export SHORTWIN_TEST_VENUE_SO=/root/fuzz/fullsha-103-20261021-a130fd98/bin/shortwin-test-venue.so
export MOCK_CPAMM_SO=/root/fuzz/fullsha-103-20261021-a130fd98/bin/mock_cpamm.so
cargo test --manifest-path services/Cargo.toml -p shortwin-keeper-litesvm \
  --test scenarios curve_exits:: --locked -- --test-threads=2
cargo test --manifest-path services/Cargo.toml -p shortwin-keeper-litesvm \
  --test scenarios graduation:: --locked -- --test-threads=2
```

The upgrade checks used `CARGO_TARGET_DIR=/root/short.win/target` and the same
explicit SBF paths with `cargo test -p shortwin-litesvm-tests --test upgrade_it
--locked -- --test-threads=2` and `cargo clippy -p shortwin-litesvm-tests
--test upgrade_it --locked -- -D warnings`.

## Independently verified final frozen inputs

Run directory: `/root/fuzz/ready-103-20261021-e04a6776`.
Metadata records the exact candidate, all-profile execution, two workers,
400 actions, seed **103410000** and a requested duration of 24 hours.
`running.json` records start **2026-10-20T22:21:09.024380+00:00**.
These records identify the launch and inputs; they are not completion evidence.

| Frozen artifact | Bytes | Independently computed SHA-256 |
|---|---:|---|
| Release `shortwin-release.so` | 1,644,024 | `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc` |
| Test venue `shortwin-test-venue.so` | 1,675,056 | `0a405deba60a521798820985cd4cec1a0e61e46ed9d0ee829fd037b010565a01` |
| Mock `mock_cpamm.so` | 281,240 | `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9` |
| Runner `shortwin-fuzz` | 11,686,272 | `5b6971e3fd11f8c8d3166d010631b40e0dd57b797f311ba97a348a35e1036b48` |

All agree with `metadata.json`. Canonical SBF files also match this table, including
the earlier complete-digest SBF bytes. The only program source change since `f5a8ccef`
is the unused named venue baseline constant; identity is established by the actual
artifact hashes, not inferred solely from that source observation.

This reviewer hashed all **334 tracked frozen fixture files**, verified their sizes
and recorded hashes, and compared their bytes/path set with the candidate source
archive and Git tree. The manifest has **344 managed venue entries**, plus one
ungrouped legacy record. Declared build-input hashes match committed bytes.
Reproducing `git archive --format=tar e04a6776f1c2f5e02e581d76dd1255c416270275`
produced the exact saved `source.tar` hash. All 90 original AI3 archive checksums
also passed in this documentation pass.

| Frozen record | SHA-256 |
|---|---|
| `metadata.json` | `3cb472ecc4a911394e4e6cbe40299ca6486a9f780c6790d87bf8ff57bc772e4c` |
| `running.json` | `85f3347ab8931f629d1cd2c14b6af461f0adb0f2c09638724ec7d5fb917e3aec` |
| `source.tar` | `caf9ce12131db852fea0e21b179fb997e1a731ed89933957c31a3e2715770840` |
| `fixtures.sha256.json` | `d0205300e4fac878a45322814047da1c578d893d353ce75517a8ac3d56e5e6ef` |
| `fixtures/manifest.json` | `f0d87f1c201b2df8016eb31648c533dc50f1832243ba84f16fdfe400166ddc4b` |
| `fixtures/SHA256SUMS` | `b17a50e1e1e7e91cf16f84ac8d810cb1af82bbaa064ef96f90b47f32ca2f9dfc` |

The new CURRENT changes the fuzzer's selected upstream programs through its harness.
The earlier runner/profile duration is therefore historical even though the three
SBF binaries are unchanged. It cannot be combined with this new runner/profile to
claim the final 24-hour gate.

## Conditions before release

[Full CI 39507546399](https://github.com/TodorBonchev/short.win/actions/runs/39507546399)
on the exact candidate and the new final-profile fuzz completion are pending.
This review does not label either green. Require completed ≥24-hour zero-failure
evidence identifying the frozen runner, programs and fixture/profile inputs above.

Before first deployment, recheck live venue pins and any renewed R-PFEE condition,
compatible guard clients and indexer sweep-destination support, and funded ordinary
keeper operation with monitoring. The never-deployed statement removes deployed-state
migration; this review has not verified a funded operator exists or authorized a tag
or deployment.

The owner's mechanism-specific economic acknowledgements remain policy decisions,
not technical fixes or guarantees of income, historical pro-rata allocation, eventual
claim payment, external keeper profitability or timely deposit reopening. Other
unresolved leads retain their individual written treatment. Acceptance here is of
the scoped integration and recorded evidence, conditional on the remaining gates.
