# Draft 1.0.3 final-source readiness (2026-10-21)

Integrated source candidate: **`a130fd9836508e0f58bc94eb00bb1f5eae95405e`**.
Complete-digest correction: **`f5a8ccefa92755661964e63d41c458fb1072e9d5`**.
Owner-selected policies and disclosures: `1a7a0d37`. The integration retains the
concurrent phone/narration demo work through `6e602f8e`.

The owner confirmed the program has **never been deployed**. No deployed-account
migration is required. No release tag or deployment has been made by this work.

## Completed corrections, decisions and review

VenueGuard now stores and compares four complete 32-byte SHA-256 digests, in a
304-byte account. No old-layout migration was added. Signed regressions demonstrate
that a matching old prefix cannot conceal a changed suffix, wrong later-slot hashes
are rejected, correct full hashes clear the latch, and pure extensions remain allowed.
The synthetic suffix test does not claim a cryptographic collision was found.

The IDL, generated client, keeper reads, layout model and browser decoder agree on
the new schema. Program and pump browser vectors were regenerated from signed SVM
execution on the new canonical ELFs. The earlier keeper ranking/evaluation outage
corrections and automatic browser exit/rollback refresh are retained.

The [owner decision record](owner-decisions-1.0.3-2026-10-21.md) explicitly selects
the retained fee/risk, cashback, keeper-payment, first-claim pot, final-holder carry
and donation-hold policies. Their exact consequences are acknowledged, including
unrecoverable cashback, claims that may never be paid, and renewable deposit denial.
The [L10 probe](probes/ai3-l10-2026-10-20/README.md) retains a reproducible patch and
passing log. Lender/borrower/claim disclosures and the glossary reflect those policies.

The [independent AI-assisted fix review](independent-ai-fix-review-1.0.3-2026-10-21.md)
accepts the inspected implementation and discloses authorship overlap and separate
peer review. Under the owner's batch-specific authorization, this is the review route
used instead of a security firm's report. No official external audit or accreditation
is claimed. The full CI and final-byte fuzz gates remain separate requirements.

| Local verification | Result |
|---|---|
| Program native library | 83 passed |
| Keeper monitor/pool-monitor tests | 28 passed; strict keeper Clippy passed |
| New-ELF Raydium latch / layout / pump-fees extension suites | 17 / 5 / 2 passed; unchanged compute budgets |
| Independent fresh-ELF signed suffix regression | Passed; overlaps the 17-case latch suite |
| Independent governance instruction tests | 4 passed |
| Generated client check | 30 passed, including full 304-byte roundtrip and byte 303; typecheck and generation consistency passed |
| Whole-web unit suite | 1,075 passed, one intentional skip |
| Whole-web typecheck / lint after narration merge | Passed; 373 files checked |
| New-program fuzz smoke / harness self-tests | 21 / 12 passed |
| Program and affected keeper/LiteSVM strict Clippy | Passed; no lint or compute-budget increases |
| Original audit archive | All 90 SHA-256 checks passed |

These are executions within each check; overlapping cases are not unique-test totals.

## CI findings corrected after the initial full-digest candidate

- **TLS dependency policy (`b54c4384`):** CI found RUSTSEC-2026-0381/0382/0383.
  Services now resolve `webpki-roots`/`webpki-root-certs` 1.0.10 and `rustls-webpki`
  0.103.16. The indexer's explicit rustls connector uses the patched Mozilla anchors,
  preserving normal hostname/certificate verification and URL redaction. The obsolete
  0.26 wrapper is removed without an advisory exception or OS-root policy change.
  Agent `remote_scope_check` executed deny/audit (all eight lockfiles), two TLS tests,
  strict all-target indexer Clippy and common/ops library tests (10/17 passed).
  Agent `keeper_fix_review` independently inspected the dependency/API changes and
  upstream TLS behavior; no blocking issue was found. It did not claim those test runs.
- **Indexer replay recordings (`1cee0c16`):** the committed-consistency test reproduced
  the old 224-byte checkpoint decoding failure. Three signed recordings were regenerated
  from the canonical 304-byte/full-digest program; the unchanged strict test then passed.
  No decoder compatibility shortcut, padding, ignored test or weakened assertion was added.
- **Legacy keeper cleanup (`604154c3`):** never-deployed status removes the 1.0.0 V2
  migration need. The old builder, format negotiation/cache and special refusal suppression
  are removed. Current/previous/stable governed extras retries, one-step exit redirection
  and the observed-deploy simulation bank floor remain. The implementer ran 95 keeper
  units, two current-version guards, 12 graduation cases and two extras cases, plus strict
  all-target keeper/LiteSVM Clippy. A separate agent reviewed the source and found no blocker.

These executions used the then-pinned venue fixtures. The
[live preflight](live-preflight-1.0.3-2026-10-21.md) subsequently detected new pump/PumpSwap
ELFs. Their new fixture baseline and signed compatibility checks are in progress;
final CI and the final fuzzer inputs must include the resulting integration.

## Exact-ref CI and artifacts

[Full CI 39501064777](https://github.com/TodorBonchev/short.win/actions/runs/39501064777)
runs on the integrated source candidate. Its services dependency gate found new
RUSTSEC-2026-0381/0382 advisories in `webpki-roots`/`rustls-webpki`; a lockfile correction
and fresh full CI are required. No advisory exception is being added. Other jobs,
including the verifier, must not be used to describe this CI gate as green.
Downloaded verifier and Anchor release artifacts are byte-identical and match the
canonical local release below; the downloaded test-venue artifact matches too.

The earlier [CI 39391034225](https://github.com/TodorBonchev/short.win/actions/runs/39391034225)
on `007915da` completed successfully: 25 jobs passed, zero failed, one scheduled fuzz
job intentionally skipped. Its verifier and Anchor release were downloaded and found
byte-identical. That evidence predates the full-digest change and does not certify
the following new program bytes.

| Local canonical artifact | File SHA-256 | Bytes |
|---|---|---|
| Release `shortwin.so` | `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc` | 1,644,024 |
| Test-venue `shortwin.so` | `0a405deba60a521798820985cd4cec1a0e61e46ed9d0ee829fd037b010565a01` | 1,675,056 |
| Mock CPMM | `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9` | 281,240 |

Canonical builds use platform-tools v1.57, SBPF v3. Independent local hashes agree.
CI 39501064777's verifier comparison passed on `a130fd98`; a green integrated CI
result is still required after correcting the services advisories.

## Final fuzz and release gate

The [final all-profile run](fuzz-1.0.3-fullsha.md) uses frozen program and fuzzer bytes
from `a130fd98`. It must complete ≥24 hours with zero failures/anomalies before fixes
can be marked **fixed**. The earlier 13h13m FR01 run was intentionally superseded
after this program change and is retained as partial evidence. The older completed
`9b91060` run uses different program bytes. Neither satisfies the final-byte gate.

After CI and fuzz complete, record the exact evidence and final statuses, then present
the tag/deployment plan for owner review. Before first deployment verify live venue
pins/R-PFEE, funded keeper operation and monitoring, indexer support for sweep
destination `3`, and compatible 304-byte guard clients. Operator funding and live
venue state were not verified by source tests. The two remaining mobile package-age
exclusions retain their separate date/installation follow-up in TODO.
