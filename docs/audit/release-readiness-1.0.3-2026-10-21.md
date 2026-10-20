# Draft 1.0.3 release readiness (2026-10-21)

**Engineering/test candidate:** `ff85a1b0c8452cb9fad32767f7083d22377e90a3`, pushed to
`audit-1.0.3-gates`. **Release acceptance is pending full CI and final-profile ≥24 h fuzz.**
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

## Exact-ref full CI and canonical artifacts

[Retained job outcomes, byte comparisons and source-equivalence evidence](probes/ci-fullsha-2026-10-21/README.md).

[Full CI 39509811932](https://github.com/TodorBonchev/short.win/actions/runs/39509811932)
is running on exact candidate `ff85a1b0`. Record its completed conclusion, all required
job outcomes, mutation result and downloaded verifier/Anchor comparison before crediting it.

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
The final exact-ref CI verifier comparison is still pending.

## Final fuzz and first-deployment gate

The [final all-profile run](fuzz-1.0.3-fullsha.md) started **2026-10-20 22:21:09 UTC**,
using source `e04a6776`, frozen programs, runner `5b6971e3…636b48` and a separate copy of
all 334 tracked fixture files. A separate reviewer matched every copy and the source
archive to the commit. It must complete ≥24 h with zero failures/anomalies and retained
coverage/exit/summary hashes before implemented findings become **fixed**. The 47-minute
old-venue/full-digest run and 13h13m pre-digest run are explicitly superseded partial
evidence; older completed runs do not satisfy this final-profile gate.

The current live-pin check passes at slot 459,391,399; pump-fees bytes still match the
retained R-PFEE analysis. Recheck before actual deployment/revalidation: this is a dated
observation, not a promise of future mainnet state. After CI and fuzz pass, record final
statuses/release hashes and present the tag/deployment plan for owner review. First
deployment also requires a funded, monitored keeper for zero-reward eligible exits,
indexer support for sweep destination `3` and compatible 304-byte guard readers. Tests do
not establish funded service operation. Later/conditional tasks remain in
[TODO](../../TODO.md); the mobile package-age follow-up is after October 21.
