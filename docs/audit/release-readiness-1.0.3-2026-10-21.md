# Draft 1.0.3 release readiness (2026-10-21)

**Current tested code candidate:** `56f660027f88ee60a33b9d2b5da516daccc9c9b9`, integrated from
`audit-1.0.3-gates`. **Full CI and final-profile ≥24 h fuzz passed; deployment preflight remains.**
No 1.0.3 tag or deployment has been made by this work.

The owner confirmed the program has **never been deployed** and expressly delegated
the recommended choices in the [owner decision record](owner-decisions-1.0.3-2026-10-21.md).
No deployed-account migration is required. The policy acknowledgements preserve their
exact economic consequences, including unrecoverable cashback, claims that may never
be paid, final-holder unallocated carry and renewable new-deposit denial.

## Completed corrections and integration

- **Full-digest VenueGuard (`f5a8ccef`):** four complete 32-byte SHA-256 fingerprints,
  304-byte allocation and matching IDL/generated client/keeper/browser readers. Signed
  suffix regressions distinguish equal old prefixes from full digests. Pump/PumpSwap
  remain exact-slot-latched at their 10 MiB allocation; they are not the four stored
  extensible-venue fingerprints.
- **Keeper and browser recovery:** retained main's AI3-FR01 empty-carry quarantine,
  keeper candidate-evaluation RPC recovery and automatic exit/rollback history refresh.
  No historical carry is guessed or reassigned to a new cohort.
- **TLS CI repair (`b54c4384`):** `rustls-webpki` 0.103.16, `webpki-roots` and
  `webpki-root-certs` 1.0.10 address RUSTSEC-2026-0381/0382/0383. The explicit rustls
  connector uses the same patched Mozilla anchors without the obsolete 0.26 wrapper,
  retaining certificate/hostname verification and URL redaction. No new advisory ignore
  or OS-root policy change.
- **Indexer evidence (`1cee0c16`, `82b3eb06`):** three signed recordings regenerated
  for the new guard layout; unchanged strict consistency check passes. Actual new pump
  TradeEvent bytes preserve the old prefix and append eight uninterpreted zero bytes;
  existing decoder handles them, with new economic-row and refusal regressions.
- **Never-deployed cleanup (`604154c3`):** old keeper V2 builder, format negotiation/cache
  and special refusal suppression removed. Current/previous/stable governed extras,
  one-step exit redirection and observed-deploy simulation bank floor retained.
- **Current-schema upgrade coverage (`ec25193d`):** real signed loader upgrade preserves
  the 304-byte guard, all four complete digests, config, live positions and vaults;
  latched opens refuse, existing repayment works, exact-slot review restores opens.
  Historical 1.0.0 position-exit evidence remains explicitly separate from migration.
- **Live venue refresh (`e04a6776`):** additive pump/PumpSwap fixtures for actual October 19
  deploys observed October 21 local; supported SOL/WSOL signed paths and unchanged CU gates
  pass. Browser validator and signed preview vectors use the new slots. All historical
  groups remain. [Scoped review and retained logs](live-preflight-1.0.3-2026-10-21.md).

Concurrent phone/narration/promo work through `3f935329` was retained. Narration API calls
and publication were not executed by this audit work; that standalone media project is
not certified by the security review.

The independent [AIR2 fix review](independent-air2-fix-review-1.0.3-2026-10-21.md)
accepts the eight scoped accounting, observation, vesting and depth corrections.
Review found the original AIR2-01 allowance assertion was masked by L39's cap.
The test-only `ff85a1b0` adds direct stored/effective lock checks and a zero-lock
control. Candidate passes; restoring the old floor fails. Production, fuzzer and
fixture inputs remain identical to `e04a6776`; the active fuzz run is not restarted.

## Concurrent service/build update (October 22 publication)

Remote `56f660027` was preserved when publishing the final audit record. It changes
keeper event authentication, duplicate-stream indexer checkpoints and locked/pinned
build controls, with no program/fuzzer/fixture change. Its exact
[CI 39556305501](https://github.com/TodorBonchev/short.win/actions/runs/39556305501)
passed 24 jobs; scheduled fuzz and push-event mutants were intentionally skipped.
The earlier `ff85a1b0` manual full gate's 19 mutant kills remain separate evidence
for unchanged mutant/program inputs. New service review and actual build-byte
comparisons are retained in the [final release record](release-checks-1.0.3-2026-10-22.md#concurrent-servicebuild-integration).
The frozen 24-hour run covers unchanged on-chain inputs, not changed service behavior.

## Attributed verification

| Check | Actual evidence |
|---|---|
| Full-digest program/keeper/layout review | Earlier focused local runs: program 83, keeper monitor 28, Raydium latch 17, layout 5, pump-fees extension 2, governance 4; client 30. These are attributed in the focused report, not claimed as new unique executions on every later commit. |
| TLS/dependency repair | Implementer ran deny and audit across all eight locks, TLS 2, common 10/ops 17 and strict indexer Clippy; separate reviewer inspected dependency/API/TLS behavior. |
| New event regression | 13 venue decoder cases and strict indexer Clippy passed; no production parser change. |
| Legacy keeper cleanup | Keeper 95 units, current guards 2, graduation 12, extras 2 and strict keeper/LiteSVM Clippy passed. Separate source reviewer found no blocker. |
| Promoted upstream fixture profile | Implementer: 45 signed integration cases; new curve differential: 180 transactions/79 matching refusals; new caller/clock controls, fixture/build-profile checks passed. Separate reviewer ran promoted keeper guards 2/graduation 12 and independently reconstructed fixture hashes. |
| New-profile performance | M5 seven and M6 three passed; fresh Cargo-linked current harness, unchanged budgets/results files. |
| Own-program upgrade | Both historical exit-only and new current304 upgrade cases passed together; strict scoped Clippy passed. Root independently inspected the new test. |
| Web after signed vector regeneration | 1,075 passed, one intentional skip; 91 passing files, one skipped file. Whole typecheck and genesis Biome passed. JSON formatting preserved parsed content. |
| New-profile fuzzer qualification | 12 harness self-tests and 21 release smoke/corpus tests passed before launch. |
| Scope/archive | Scope check passes; original 90-file review archive hashes unchanged. |

Executions overlap. Counts are not unique-test totals or an exhaustive audit claim.
The [focused independent AI review](independent-ai-fix-review-1.0.3-2026-10-21.md)
pins the complete-digest implementation; the
[integration review addendum](independent-integration-review-1.0.3-2026-10-21.md)
records later source/evidence, author overlap and separate peer review. The owner's delegation and request for an agent-created review are implemented through
this accurately labelled AI-assisted batch-specific route. No official external audit, accreditation or signature
is claimed; CI, reproducible builds and final-profile fuzz remain required.

## Exact-ref core gate and canonical artifacts

[Retained job outcomes, byte comparisons and source-equivalence evidence](probes/ci-fullsha-2026-10-21/README.md).
The later exact-`56f660027` [service/build CI](https://github.com/TodorBonchev/short.win/actions/runs/39556305501)
passed 24 jobs with scheduled fuzz/push-event mutants skipped; its downloaded Anchor
release/test/mock and verifier artifacts match the same canonical bytes below.
Both new artifact ZIP digests and the actual unchanged IDL were independently checked.
[New retained comparison](probes/fuzz-final-1.0.3-2026-10-22/remote-56-review/README.md).
The earlier dispatch below supplies the separately attributed full mutation gate.


[Full CI 39509811932](https://github.com/TodorBonchev/short.win/actions/runs/39509811932)
completed **success** on exact candidate `ff85a1b0`: **25 jobs passed, zero failed,
one intentional scheduled-fuzz skip**. All 19 selected mutants were killed by tests;
seven also had the required positive-failure fuzz kills and twelve were explicitly
test-only. Actual verifier/Anchor/frozen bytes match and both ZIP digests are verified.
[Retained completed job/step evidence](probes/ci-fullsha-2026-10-21/final-ci-summary.json).

The prior [new-profile run 39507546399](https://github.com/TodorBonchev/short.win/actions/runs/39507546399)
on `e04a6776` was superseded to include the strengthened regression. It ended
**cancelled**, with 21 successful jobs, four cancellations (web-e2e, program-tests,
mutants and services), no failed jobs and one scheduled fuzz skip. Downloaded verifier,
Anchor release, test-venue and mock artifacts match the frozen final fuzz bytes.
That partial run is not credited as the final full-CI gate.

[Initial full-digest CI 39501064777](https://github.com/TodorBonchev/short.win/actions/runs/39501064777)
on `a130fd98` ended **cancelled when the new candidate was dispatched**: 21 jobs passed,
two failed (TLS policy and old replay recording), two cancelled (program-tests/mutants),
one scheduled fuzz skip. Its successful verifier and downloaded Anchor release were
byte-identical; downloaded release/test-venue hashes matched the local inputs.
Those partial job successes are not a completed integrated gate. The known failures
have the explicit corrections above. Earlier CI 39391034225 on `007915da` was green,
but predates the full-digest correction and current venue profile.

| Canonical artifact | File SHA-256 | Bytes |
|---|---|---|
| Release shortwin.so | `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc` |1,644,024|
| Test-venue shortwin.so | `0a405deba60a521798820985cd4cec1a0e61e46ed9d0ee829fd037b010565a01` |1,675,056|
| Mock CPMM | `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9` |281,240|

Canonical `anchor build --tools-version v1.57 --arch v3 --ignore-keys` and test-venue
rebuilds on the new baseline reproduce all three bytes/hashes above. The new constant
is test/tool metadata unused by the program. Release trimmed executable hash:
`f3d8dd7a5c62385abc6306e2da395ff666a1f69234ea50f2e7eb0523ab686250`;
test-venue trimmed hash `e016b8712080ef08d0b9dc1546eeda8bc62fada891623ad9f52fb90b19de042a`.
The final `ff85a1b0` verifier, Anchor release and frozen release are byte-identical;
Anchor test-venue/mock match too. Both downloaded ZIP digests match GitHub metadata.
[Independent actual-byte comparison](probes/ci-fullsha-2026-10-21/final-ff85-verifier-comparison.json).
The complete final CI gate is green.

## Final fuzz and first-deployment gate (updated 2026-10-22 local)

The [final all-profile run](fuzz-1.0.3-fullsha.md) completed naturally on the frozen
`e04a6776` runtime: exit 0, supervisor 86,428.074 seconds and runner 86,426.898 seconds;
38,563 runs, 87,632,843 transactions, zero invariant failures/anomalies. Exact final
summary/log hashes, all four binaries, source archive, 334 fixtures and the six-profile
scheduling witness passed independent verification. [Retained evidence](probes/fuzz-final-1.0.3-2026-10-22/README.md).
Earlier partial and different-program runs remain superseded. Technical finding
closure does not remove acknowledged claim, carry, fee or keeper-payment risks.

The [October 22 preparation record and concrete tag/deployment plan](release-checks-1.0.3-2026-10-22.md)
record passing scope, fixture-integrity, package-age install, client and web checks.
The read-only live check at slot 459,802,698 **failed solely on Jupiter's new deploy
459,479,624** versus baseline 454,465,850; all other monitored bytes, including
pump-fees, match. Jupiter is outside current execution masks, but the keeper watches
it and configured initialization slots must be reviewed. No automatic repin or
upstream promotion was made. The aggregate check is not green.

Actual funded/monitored keeper operation, covered markets/top-ups/alerts and running
indexer/client identities remain unverified. Compatible source/tests do not supply
operating evidence. These prerequisites and the owner's final review remain before
any tag or first deployment; no tag, deployment or transaction was executed here.
