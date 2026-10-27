# Successful release-preparation CI — 40434130363

[Full workflow](https://github.com/TodorBonchev/short.win/actions/runs/40434130363)
completed success on exact source `57bc1c8454a89a303694e2ea8d9d8df11c0887da`:
**25 passing jobs, zero failed, one intentional `fuzz-nightly` skip**. The manual
dispatch ran mutation checks: **19 unique selected mutants killed**; seven also
failed their selected fuzz profile, twelve are explicitly tests-only. The raw log
prints the summary twice; `final-verification.json` deduplicates identical named
rows rather than counting 38 results.

`run.json` and `jobs.json` retain completed API state and individual steps. The
services job passed formatting, both Clippy paths, dependency checks, unit/SVM/
governance tests and its release A49 check. Its exact retained log proves both
new startup-credential regressions executed and passed; the separate JSON records
their actual lines. Earlier failed/superseded runs remain separately recorded.

Both downloaded artifact ZIP SHA-256 values match GitHub's own digests. Independent
extraction/rehashing proves Anchor and verifier release files both equal canonical
`d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc`
(1,644,024 bytes), with unchanged test-venue/mock artifacts. The actual CI IDL
hash equals the committed client IDL. The verifier build log and hashes are
retained as exact gzip bytes; see `artifact-verification.json` for all file and ZIP
identities. Original ZIP downloads remain local under the matching target record.

This CI uses the existing Token-2022 11.0 fixture. It qualifies the preparation
source and host patches, not the new 11.1 upstream ELF. New fixture/clock/ctutils
baseline work is separate; no old duration is credited to it. The final mainnet tag,
new baseline execution/24-hour evidence and real funded operator commissioning
remain distinct gates. A following audit-receipt-only commit does not change these
program/service/fuzzer/fixture inputs; its source-equivalence check is retained.
