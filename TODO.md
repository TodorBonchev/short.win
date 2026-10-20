# Pending work

This file lists unfinished work only. Completed audit and release evidence is in
[the 1.0.3 handoff](docs/audit/handoff-1.0.3-2026-10-20.md),
[the findings tracker](docs/audit/findings.md), and
[the validation record](docs/audit/ai-review-3/validation-1.0.3.md).
The 1.0.3 candidate is unreleased; do not tag or deploy it from this checklist.
The 2026-10-20 integration also includes `main`'s AI3-FR01 deposit quarantine.
The completed fuzz record below covers the earlier `9b91060` program, not those
new bytes. The owner's confirmation that no affected deployed state exists makes
current-deployment FR01 recovery/migration not applicable.

## 1.0.3 release blockers

- [ ] Review the handoff's keeper ranking-retry change and the remaining
      candidate-evaluation RPC error path. Add a focused regression that shows
      whether `pool_fail` observations continue during a sustained evaluation
      read failure; fix that path if needed.
- [ ] Run the full gate and exact-ref GitHub CI on the new handoff commit.
      Reconfirm verifiable/Anchor ELF equality, keeper scenarios, browser E2E,
      dependency policy and mutations. The prior [green CI run](https://github.com/TodorBonchev/short.win/actions/runs/39144291500)
      covers `9b91060`, before this keeper change.
- [ ] Run the ≥24 h all-profile fuzz gate on the final AI3-FR01 program and
      fuzzer bytes. The [completed clean run](docs/audit/fuzz-1.0.3.md) identifies
      the older `9b91060` program and does not cover the integrated quarantine.
- [ ] Obtain an external auditor's fix review for the AIR2 and AI3 batches and
      the keeper follow-up. The [internal review handoff](docs/audit/handoff-1.0.3-2026-10-20.md)
      is reviewer input, not the sign-off required by
      [AUDIT §12](docs/AUDIT.md#12-logistics-and-change-policy).
- [ ] Resolve or get explicit owner-signed, bounded acknowledgements for
      AIR2-07, AIR2-10/11, AIR2-12, AIR2-13 and AIR2-P2-02. AIR2-12 and
      AIR2-P2-02 have confirmed fund-allocation traces; AIR2-05's canonical
      sweep fix does not recover alternate-account cashback.
- [ ] Decide the newly reproduced AI3-F06 carry-ownership residual: a lender
      depositing one raw token after income vested can receive an earlier
      lender's whole-lamport carry on the final exit. The math and signed
      LiteSVM characterizations are in the handoff. Choose bounded
      final-holder allocation or redesign before declaring the fix complete.
- [ ] Disposition the remaining AI3 leads, especially L10's repeat-donation
      deposit hold, L13/L19–L21 fee/supply crossings, L16 tier-allocation
      bound, L22 cashback claim liveness and L23's exact KI-68 comparison.
      Carry forward the substantive AI2 configuration/economic leads in the
      [triage registers](docs/audit/ai-review-3/triage.md).
- [ ] Only after the review and owner decisions, update
      `docs/audit/findings.md` and `docs/audit/known-issues.md` to the
      correct formal statuses and bounds. Confirm release hashes and
      deployment order (indexer before program for sweep destination 3),
      then have the owner review the 1.0.3 tag and deployment.

## Later or conditional follow-ups

- [ ] Upload the [local-validator demo](apps/web/e2e/demo/README.md)'s MP4 as
      an unlisted video, add its URL to the site/pinned post and share the
      10-second GIF. The recorded files are local and gitignored; the scripts
      can regenerate them from a fresh validator seed.
- [ ] Investigate exit-history refresh when the Liquidations tab opens
      before a keeper event reaches the indexer. In the guided demo rehearsal,
      the tab stayed empty for 45 seconds while the API already contained the
      exit; reloading showed it. The demo waits for ingestion before opening
      the tab. Also check the transient pre-close status retained on the
      closed-position screen.
- [ ] Complete the original AI review 2 pass-2 judge run, pass 3 and an
      independent keeper-core review if that broader audit scope is resumed.
- [ ] After 2026-10-21, remove the two remaining `@solana-mobile`
      release-age exclusions in `pnpm-workspace.yaml` and rerun
      `pnpm install --frozen-lockfile`.
- [ ] On any new pump-fees ELF, re-read `withdraw`'s accounts and authority
      (R-PFEE) before relying on the 2026-10-12 review.
- [ ] Remove the keeper's legacy `pump_trade_extras_v2` builder when the
      post-1.0.1 migration window is over.
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
