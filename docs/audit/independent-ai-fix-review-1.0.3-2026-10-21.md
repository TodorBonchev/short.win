# Independent AI-assisted fix review for draft 1.0.3

Date: 2026-10-21. Reviewer: Codex review agent `keeper_fix_review`, with a
separate AI peer review of the keeper change this reviewer previously authored.
**Focused implementation outcome: accept the complete-digest correction and the
bounded keeper recovery behavior described below. Release acceptance remains
conditional on the remaining integrated CI, verifier and final-byte fuzz gates.**

## Provenance and independence

This is agent-authored code review and executable verification. It is not a report
from an external security firm, an accredited audit, a professional signature,
a deployed-state assessment, or an assertion that all economic risks are fixed.
The owner explicitly requested this review route and delegated the policy choices
recorded in [the dated owner decisions](owner-decisions-1.0.3-2026-10-21.md).
That record is the authority for the batch-specific review-route exception;
the tests below are technical evidence and are not the source of owner consent.

This reviewer did not author the new complete-digest production implementation,
its unit regressions, or the regenerated client/browser layout regressions.
The reviewer read the production diff, surrounding control flow, account
constraints, readers and governance encoding independently of the implementer's
conclusions. Implementation test claims were input to review, not substituted
for inspection or silently reported as this reviewer's executions.

There are two disclosed overlaps with earlier work in this conversation:

- This reviewer authored the keeper candidate-evaluation RPC-error branch and
  its two scenarios. A separate agent, `remote_scope_check`, which did not
  author that change or those tests, independently inspected that scope on
  2026-10-21. Its peer conclusion is attributed below; it did not rerun the suites.
- This reviewer authored the signed AI3-FR01 adversarial regression fixture,
  not the production math quarantine. Earlier independent accounting review
  and the old-ELF negative control remain separately attributed evidence.

Earlier AIR2/AI3 findings and accepted technical corrections retain their original
scope and evidence limits. This is a focused fix review with explicit reliance
on [the earlier independent packet review](ai-review-3/independent-fix-review-1.0.3.md),
[accounting review](ai-review-3/independent-accounting-1.0.3.md),
[keeper review](ai-review-3/independent-keeper-1.0.3.md) and
[reader review](ai-review-3/independent-readers-1.0.3.md). It is not a new exhaustive
audit of every AIR2 lead, the frontend, upstream venues, or the whole protocol.

## Source and integration identity

The reviewed complete-digest source range is
`007915da9d9c7a764783683f38bd3f4b961e6398` →
**`f5a8ccefa92755661964e63d41c458fb1072e9d5`**.
Review and execution began on the uncommitted diff, then the implementation was
committed at that immutable endpoint. This reviewer independently verified the
final commit and an empty working-tree diff against it for the program/math,
keeper, services readers/governance, generated guard IDL/codec, client/browser
guard tests and LiteSVM sources. The four source hashes below still match.
The normative layout and keeper operating-description updates also match the
304-byte/full-digest schema. The previous `eb9cd3e6` release and `cf1c4442`
test-venue binaries predate this program change and cannot establish final-byte
acceptance of it. Subsequent documentation or unrelated UI integration requires
its own scope checks; this report does not certify unreviewed later runtime edits.

The reviewed core source has SHA-256:

| File | SHA-256 |
|---|---|
| `programs/shortwin/src/state/venue_guard.rs` | `49672f303e587b2f8f9b4b0702fd7681c6a3dacd33f4c0a67d26ba185d107cf7` |
| `programs/shortwin/src/instructions/flag_venue_upgraded.rs` | `2e2f776f1916fae9311a5c794c8aab0f846de5cba0bbe66903873453115fcde7` |
| `clients/js/idl/shortwin.json` | `5b94b1e98f5b32cf6d3c501d1998965bbbab80d6e2853fa6eeb8f3ba5470a003` |
| `clients/js/src/generated/accounts/venueGuard.ts` | `71dd94e2cd77adf32de4c07c8097d3821a5ba8dee15fa5f256f5a783c7f84499` |

No repository `AGENTS.md` was found. Only this report is written by this review
pass; the runtime implementation, client generation, ELF builds and release
record integration belong to other agents.

## Complete-digest production review: AIR2-07 / AI3-L12

**Accept the inspected implementation. No additional blocking code defect was
identified in this focused scope.** The removed 12-byte comparison is replaced
with storage and equality of all 32 SHA-256 bytes. The following are the actual
mechanisms reviewed:

| Surface | Review conclusion |
|---|---|
| Account storage and allocation | `GUARD_ELF_LEN = 32`; `elf_fingerprint_of` returns the entire input array. Four digest entries occupy bytes 176–303 inclusive. `VenueGuard::SPACE` is 304, including the discriminator; the struct body is 296 bytes. The prior fields keep their offsets. Compile-time layout assertions and `initialize_config`'s `space = VenueGuard::SPACE` match this schema. |
| Upgrade flag | ProgramData address, owner and header remain authenticated. A moved slot with unchanged recorded length still counts as an upgrade. A changed-length account is treated as a pure extension only when its complete trimmed-ELF digest equals the complete stored digest. Oversized unhashable ELF remains fail-safe. The slot/error/latch ordering is unchanged; digest growth does not bypass the existing slot prerequisites. |
| Revalidation | The instruction argument was already `[u8; 32]`. On an unexpected deploy slot, the full live digest must equal that argument before any guard slot, digest or latch write. Exact-slot governance continues to trust the supplied reviewed hash, as before; a mistaken supplied suffix subsequently causes a fail-safe extension flag. The correction does not claim to newly authenticate that trusted input. |
| Native keeper | `ElfFacts`, stored flagged records and equality use `GUARD_ELF_LEN` and therefore all 32 bytes. The keeper calls the program's trimmed-hash helper. `watch_programs` requests `max(HEADER_LEN, VenueGuard::SPACE)`, now 304, and typed decoding checks owner, discriminator, size and supported version. A truncated old 224-byte guard is not accepted as a new complete guard. |
| Governance byte mapping | `services/ops/src/ixs.rs::revalidate_venue` carries the full existing 32-byte instruction argument and selects the expected ProgramData account. The review description renders 64 hex digits. No instruction-argument truncation or instruction ABI change was found. |
| IDL, JS and browser | Regenerated IDL and codec contain four 32-byte entries and size 304. The client roundtrip reaches byte 303; the browser snapshot preserves all four digest suffixes and the preexisting upgrade-status fields. Both reject a truncated 224-byte account in the executed regressions. |

The discriminator and guard version remain unchanged. This is acceptable for the
owner-confirmed **first deployment**, with mutually compatible program, keeper and
client versions. It is a changed account layout, not backward-compatible decoding
or an upgrade migration for existing 224-byte accounts. The owner's statement that
the program has never been deployed removes that migration prerequisite; it does
not establish compatibility with old published readers or eliminate future
economic risks. Coordinated release artifacts remain required.

The suffix regressions deliberately use synthetic recorded digests with equal
first 12 bytes and different suffixes. They demonstrate that storage/comparison
no longer discards the suffix. They do not exhibit two colliding executable ELFs,
prove SHA-256 collision resistance, or quantify an executable chosen-pair attack.

The new LiteSVM test
`air2_07_a_matching_digest_prefix_does_not_clear_or_hide_a_changed_suffix`
was independently read. Its exact-slot synthetic review mismatch, extension
flag, blocked-open check, stale-slot wrong-hash rejection and correct-hash
revalidation are appropriate signed-path checks. This reviewer also executed that
one test on the independently hashed fresh artifacts below: one passed, zero
failed, 16 filtered, with 1.84 seconds of test execution. Cargo compiled the test
crate from the review checkout. This is a signed LiteSVM path, not a mainnet test.

The fresh artifacts used for that execution were independently hashed:

| Artifact | SHA-256 |
|---|---|
| Release `target/deploy/shortwin.so` | `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc` |
| Test-venue `target/test-venue/deploy/shortwin.so` | `0a405deba60a521798820985cd4cec1a0e61e46ed9d0ee829fd037b010565a01` |
| Mock CPMM `target/deploy/mock_cpamm.so` | `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9` |

This establishes the actual local test inputs. It does not assert an independent
clean rebuild or verifier equality for those artifacts.

## Earlier technical corrections and integration limits

| Correction | Technical disposition and evidence boundary |
|---|---|
| Implemented AIR2 fixes | Existing focused regressions and prior triage remain evidence for their named predicates. This localized guard diff does not replace the required integrated CI, final-byte fuzz or review of the earlier implementation scope. The economic rows below are not included in a claim that AIR2 is wholly fixed. |
| AI3-F01–F05, L01/L03/L04 | Retain the earlier focused implementation acceptance: program-matching candidate floor, continued eligible-candidate search, creator placeholder resolution, raw LP ranking, coherent complete rank batches, typed version refusal and complete ALT headers. The earlier reports identify their candidate revisions and verified artifacts. Current integrated execution evidence must be associated with its own source/artifact identities. |
| AI3-F06 and AI3-FR01 | Accept the narrow correction: final withdrawal assigns whole-lamport carries according to the selected final-holder rule, and a deposit into an already-empty carry-bearing pool refuses 6110 before touching/reassigning historical value. Allocated old claims remain. This does not reconstruct historical pro-rata carry ownership. The owner-confirmed absence of deployed state removes present migration work; future encountered legacy carry would still require reviewed recovery. See the [quarantine record](ai-review-3/ai3-fr01-fix-1.0.3.md). |
| Ranking and evaluation RPC recovery | Accept the bounded recovery behavior. Ranking publishes/dequeues a batch only after all groups succeed. Evaluation transport/short replies preserve the current incumbent's observation chain only after this tick independently reads/projects it as failing with zero positions; they return before candidate planning/proposal and do not advance successful-evaluation time. Healthy or unreadable incumbents do not acquire new failure evidence. This reviewer authored the evaluation change, so the independent conclusion for that branch is the peer review described next. |

The separate peer reviewer independently traced `select_market`'s fresh incumbent
read/projection, the failed/incomplete evaluation path and both scenarios. It found
no blocking issue: an evaluation failure cannot supply fresh sibling eligibility,
cannot turn a healthy incumbent's old chain into current failure, and cannot
create a current-pool observation when its view is unreadable. The scenarios span
more than the fast-route interval, require at most 30-second crank gaps and
unchanged chain origin, prohibit stale proposals, and recover with fresh reads.
This is an AI peer source review of that branch and those tests, not an external
audit or a new execution claim.

This reviewer previously executed the 18 pool-selection scenarios and strict
keeper/keeper-LiteSVM Clippy on the preceding FR01 artifacts. Those passed, but
they are historical evidence for unchanged keeper recovery source and the old
program bytes. They are not a new full scenario run on the complete-digest ELF.
The earlier meaningful negative control failed on a 31-second observation gap
before the evaluation recovery fix.

## Checks actually executed in this review pass

All commands ran through WSL Ubuntu-24.04 from
`/root/short.win-audit-review`. Existing native target caches and installed JS
dependencies were reused. Successful Cargo invocations execute tests associated
with the current checkout; the program-lib run reused the current compiled test
binary and is not claimed as an independent clean build.

```sh
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/root/short.win/target \
  cargo test -p shortwin --lib --locked

CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/root/short.win/services/target \
  cargo test --manifest-path services/Cargo.toml \
  -p shortwin-keeper --lib monitor::tests --locked

CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/root/short.win/services/target \
  cargo test --manifest-path services/Cargo.toml \
  -p shortwin-ops --lib ixs::tests --locked

CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/root/short.win/target \
SHORTWIN_SO=/root/short.win-audit-review/target/deploy/shortwin.so \
SHORTWIN_TEST_VENUE_SO=/root/short.win-audit-review/target/test-venue/deploy/shortwin.so \
MOCK_CPAMM_SO=/root/short.win-audit-review/target/deploy/mock_cpamm.so \
  cargo test -p shortwin-litesvm-tests --test latch_raydium_it \
  air2_07_a_matching_digest_prefix_does_not_clear_or_hide_a_changed_suffix \
  --locked -- --exact --nocapture

(cd docs/audit/ai-review-3 && sha256sum -c SHA256SUMS)

export PATH=/root/.local/share/mise/installs/node/24.21.0/bin:/root/.local/share/mise/installs/pnpm/12.7.0:/usr/local/bin:/usr/bin:/bin
(cd clients/js && node --test test/layout.test.ts)
pnpm --filter web exec vitest run src/features/short/venueGuard.test.ts
```

| Check | Actual result |
|---|---|
| Program lib | **83 passed**, zero failed; includes suffix-sensitive digest storage/extension cases and layout/error tests |
| Keeper `monitor::tests` filter | **28 passed**, zero failed, 71 filtered; the substring also selects `pool_monitor::tests`, so this is not a claim of 28 venue-monitor-only cases |
| Ops `ixs::tests` | **4 passed**, zero failed, 13 filtered; includes complete hash representation in governance review |
| Fresh signed full-digest regression | **1 passed**, zero failed, 16 filtered on the fresh artifacts above |
| Original AI3 archive hashes | **90 files passed** `sha256sum -c`; historical archive unchanged |
| Client layout | **18 passed**, zero failed; size 304, all asserted layouts, full-digest roundtrip and old-size refusal |
| Browser guard snapshot | **2 passed**, zero failed; complete suffix/status decoding, truncated/foreign account refusal |

The initial JS attempts did not run tests because the WSL login PATH lacked Node,
then an inherited Windows PATH was parsed incorrectly. The successful commands
above use an explicit Linux PATH. No failed assertion was suppressed by that
environment correction. This reviewer has not independently executed a
mutation/old-prefix negative control, clean verifier build, full integrated CI,
or a completed final-byte 24-hour fuzz run.

## Owner acknowledgements: distinct from implementation fixes

The owner selected the following policies under express delegation. This reviewer
supports recording them with these exact limitations; consent is not technical
proof that an issue is harmless or fixed. The detailed rationale and operating
requirements remain in [the owner decision record](owner-decisions-1.0.3-2026-10-21.md).

| Item | Retained mechanism and acknowledged consequence |
|---|---|
| AIR2-10/11 and associated fee leads | Capped risk assessment and actual live execution fees remain distinct to preserve sale-manipulation protection and quote the actual CPI. Creator/fee/supply changes may affect risk decisions and exit affordability. No maximum harmful-decision loss or universal executable-exit deadline has been established. |
| AIR2-12 | Canonical authenticated sweeping remains. Permissionless diversion can strand potentially all repeatedly diverted future unbooked cashback; the 545,713-lamport fixture is a sample, not a cap. Complete discovery/recovery/historical attribution are absent. This is reward-loss/completeness risk, not demonstrated lending-token principal theft. |
| AIR2-13 | Rent-first allocation can leave zero equity for the keeper even with a nominal floor. The existing keeper attempts program-eligible exits without a reward-versus-fee profitability gate; these exits are outside its observation budget, although simulation, priority and per-slot CU limits still apply. A funded operator backstop is a first-deployment operating prerequisite; rational unaffiliated keeper service at a loss is not guaranteed. This review does not verify a funded service exists. |
| AIR2-P2-02 | First-claim access to unmatched cash and current-share shortfall accounting remain. Order-dependent cash priority can leave later positive claims reliant on deficit covers that may never arrive. This is not merely delayed payment; historical pro-rata and eventual-payment guarantees are absent. |
| AI3-F06 ownership residual | A late holder can receive pre-entry unallocated whole-lamport carry. Approximately 0.018447 SOL per-index representation bounds depend on the documented ratio/u64-supply assumptions, are not a lifetime cap, and cannot be added as independent entitlement/realised cash losses. Allocated claims are separately preserved. |
| AI3-L10 | Actual-vest admission isolation remains, preserving AIR2-04/05. Renewable denial of new deposits is acknowledged; old-owner claims/withdrawals remain. The [retained production probe](probes/ai3-l10-2026-10-20/README.md) extended the hold by 219,140 seconds with 78 donated lamports and 155,000 fee lamports. No universal reopening time, duration or cost bound follows from this finite trace. |

This reviewer executed the retained L10 characterization on the preceding frozen
FR01 production binary. Its patch/log/hashes and atomic signed-transaction method
are preserved in the linked probe; the temporary tracked test was restored.
It is characterization evidence, not a permanent G-full regression or an assertion
that the new complete-digest artifact was used.

Other unresolved AI2/AI3 leads retain their explicit individual dispositions.
In particular, economic acknowledgement does not turn unmeasured maximum fee-tier
resource limits, upstream reward claimability or configuration-sensitivity leads
into tested safety guarantees. Never-deployed status removes historical migration,
not future cross-cohort allocation or availability consequences.

## Release-gate conclusion

The focused complete-digest implementation is technically acceptable on the
inspected source, with no additional blocking defect identified. Economic
acknowledgements are owner policy choices with the explicit consequences above.
The narrow legacy quarantine and bounded keeper recovery are accepted with their
provenance and peer-review distinctions.

This report pins the reviewed implementation to `f5a8ccefa92755661964e63d41c458fb1072e9d5`
and records the actual fresh artifacts used for the independently executed signed
suffix regression. Full exact-ref CI, verifier comparison and at least 24 hours
of zero-failure fuzz on the final program/fuzzer bytes remain separately required.
The guard change invalidates reuse of the old
FR01 program's fuzz duration as final-byte evidence. This report grants no tag,
deployment approval, external-firm certification or blanket closure of findings.
