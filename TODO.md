# Pending work

This file lists unfinished work only. Completed audit and release evidence is in
[the integrated devnet preparation record](docs/audit/release-wrap-up-1.0.3-2026-10-22.md),
[the 1.0.3 handoff](docs/audit/handoff-1.0.3-2026-10-20.md),
[the current readiness record](docs/audit/release-readiness-1.0.3-2026-10-21.md),
[the findings tracker](docs/audit/findings.md), and
[the validation record](docs/audit/ai-review-3/validation-1.0.3.md).
The 1.0.3 candidate is unreleased; do not tag or deploy it from this checklist.
The final candidate also includes the AI3-FR01 quarantine and the owner-selected
complete SHA-256 VenueGuard correction for first deployment.

## 1.0.3 release blockers

- [ ] Qualify the integrated candidate's final bytes: the corrected module comment
      shifts one Anchor diagnostic line-number byte in the ELF. The previous clean
      ≥24-hour run is retained for its original hash; the new candidate needs its
      own applicable pinned/verifiable build, final gates and final-byte fuzz.
- [ ] Complete or explicitly disposition the [new audit checkpoint](docs/audit/2026-10-22-progress/README.md)
      before any whole-codebase audit-complete claim. Seven independently retained
      hypotheses require validation and 23 lack independent disposition; preserve
      the two provider-stopped scope gaps without retrying those tasks.
- [ ] Resolve the [current Jupiter live-pin drift](docs/audit/release-checks-1.0.3-2026-10-22.md)
      through explicit upstream/deployment configuration review before first deployment.
      Keep the failed aggregate live check recorded; do not silently repin it.
- [ ] Verify actual funded/monitored keeper operation, covered markets, payer top-ups and
      alerts, plus running indexer destination `3` and 304-byte client artifact identities.
      Tests establish compatibility; operating evidence is still pending.
- [ ] Obtain owner review of the [concrete 1.0.3 tag/deployment plan](docs/audit/release-checks-1.0.3-2026-10-22.md)
      after the remaining preflight requirements are satisfied. Final CI, frozen ≥24 h
      fuzz and technical finding closure for the earlier canonical artifact are
      recorded there; the new integrated artifact has the additional gate above.

## Public-devnet test prerequisites

- [ ] Fund the new devnet payer with test SOL; its public address, external key path
      and the failed initial faucet request are in the
      [integrated preparation record](docs/audit/release-wrap-up-1.0.3-2026-10-22.md).
- [ ] Resolve the target-cluster profile before promising market E2E: canonical
      Raydium/Jupiter IDs are not executable there, devnet Squads fails the supported
      immutability check, and Pump dependency parity is unverified. Establish fresh
      Pyth observations and actual operator/client/indexer configurations. A
      program-only deployment does not close these requirements.

The [owner-selected policies](docs/audit/owner-decisions-1.0.3-2026-10-21.md) and
accurately labelled [focused](docs/audit/independent-ai-fix-review-1.0.3-2026-10-21.md) /
[integration](docs/audit/independent-integration-review-1.0.3-2026-10-21.md) /
[AIR2](docs/audit/independent-air2-fix-review-1.0.3-2026-10-21.md) AI fix reviews
are recorded. The program has never been deployed; old-account migration is not needed.
The batch-specific review route does not waive CI or final-profile fuzz. The live venue
refresh and legacy keeper cleanup are complete in the source candidate.

## Later or conditional follow-ups

- [ ] Upload the [local-validator demo](apps/web/e2e/demo/README.md)'s desktop
      or portrait phone MP4 (or the [56-second promo](tools/promo/README.md)) as
      an unlisted video, add its URL to the site/pinned post and share the
      10-second GIF. The recorded files are local and gitignored; the scripts
      can regenerate them from a fresh validator seed.
- [ ] Complete the original AI review 2 pass-2 judge run, pass 3 and an
      independent keeper-core review if that broader audit scope is resumed.
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
