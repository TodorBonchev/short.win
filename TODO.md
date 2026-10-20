# Pending work

This file lists unfinished work only. Completed audit and release evidence is in
[the 1.0.3 handoff](docs/audit/handoff-1.0.3-2026-10-20.md),
[the current readiness record](docs/audit/release-readiness-1.0.3-2026-10-21.md),
[the findings tracker](docs/audit/findings.md), and
[the validation record](docs/audit/ai-review-3/validation-1.0.3.md).
The 1.0.3 candidate is unreleased; do not tag or deploy it from this checklist.
The final candidate also includes the AI3-FR01 quarantine and the owner-selected
complete SHA-256 VenueGuard correction for first deployment.

## 1.0.3 release blockers

- [ ] Finish the new pump/PumpSwap fixture-baseline and signed compatibility review
      described in the [live preflight](docs/audit/live-preflight-1.0.3-2026-10-21.md).
      Retain exact observed bytes, supported SOL-quote behavior and slot-latch constraints.
- [ ] Run fresh exact-ref full CI after the TLS fix (`b54c4384`), replay fixture repair
      (`1cee0c16`), legacy keeper cleanup (`604154c3`) and venue integration. Verify all
      required jobs and compare verifier/Anchor artifacts with final fuzz inputs.
      [Initial full-digest CI 39501064777](https://github.com/TodorBonchev/short.win/actions/runs/39501064777)
      found the TLS and replay failures; its successful verifier alone is not a green gate.
- [ ] Complete the [final full-digest ≥24 h all-profile fuzz run](docs/audit/fuzz-1.0.3-fullsha.md)
      on the recorded program and fuzzer bytes; retain zero-failure/anomaly results,
      all-profile coverage, elapsed time, exit status and summary/log hashes.
      The preceding 13h13m run was intentionally superseded by the full-digest change.
      Moving the venue profile to new upstream ELFs also requires a fresh final runner/run.
- [ ] After both gates, record final finding statuses and release hashes, then present
      the 1.0.3 tag/deployment plan for owner review. Verify funded/monitored keeper
      operation, live venue pins/R-PFEE, indexer support for sweep destination `3`
      before program deployment, and compatible 304-byte guard clients.

The [owner-selected policies](docs/audit/owner-decisions-1.0.3-2026-10-21.md) and
[accurately labelled independent AI fix review](docs/audit/independent-ai-fix-review-1.0.3-2026-10-21.md)
are recorded. The program has never been deployed; no old-account migration is needed.
The batch-specific AI review route does not waive the CI or final-byte fuzz gate.

## Later or conditional follow-ups

- [ ] Upload the [local-validator demo](apps/web/e2e/demo/README.md)'s desktop
      or portrait phone MP4 (or the [56-second promo](tools/promo/README.md)) as
      an unlisted video, add its URL to the site/pinned post and share the
      10-second GIF. The recorded files are local and gitignored; the scripts
      can regenerate them from a fresh validator seed.
- [ ] Complete the original AI review 2 pass-2 judge run, pass 3 and an
      independent keeper-core review if that broader audit scope is resumed.
- [ ] After 2026-10-21, remove the two remaining `@solana-mobile`
      release-age exclusions in `pnpm-workspace.yaml` and rerun
      `pnpm install --frozen-lockfile`.
- [ ] On any new pump-fees ELF, re-read `withdraw`'s accounts and authority
      (R-PFEE) before relying on the 2026-10-12 review.
- [ ] Run the Surfpool synthetic-close fork case in
      `tests/litesvm/tests/graduation_surfpool.rs` when Surfpool is available.
- [ ] Re-measure O19's real-venue held sandwich, FITX-1 fork figures,
      graduation rows, KI-02's real-venue figures and KI-76's cost estimate
      if those historical figures are used for a new decision.
- [ ] Measure Jupiter's ProgramData ELF size and extension hash cost before
      adding Jupiter to a future venue latch mask.
- [ ] Optional: make the documented keeper deployment/alert thresholds
      configurable.
- [ ] Repair or re-record the changed-message signature in
      `raydium_fast_path_venue_review.jsonl` before treating it as signed
      INGEST evidence.
- [ ] If using the drained-pool fuzz figures to claim a risk reduction,
      measure a comparable pre-AIR baseline; the current 10-minute run alone
      does not establish that comparison.
