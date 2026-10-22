# Pending work

This file lists unfinished work only. Completed audit and release evidence is in
[the integrated devnet preparation record](docs/audit/release-wrap-up-1.0.3-2026-10-22.md),
[the 1.0.3 handoff](docs/audit/handoff-1.0.3-2026-10-20.md),
[the current readiness record](docs/audit/release-readiness-1.0.3-2026-10-21.md),
[the owner-delegated release decision](docs/audit/owner-delegated-release-decision-1.0.3-2026-10-22.md),
[the findings tracker](docs/audit/findings.md), and
[the validation record](docs/audit/ai-review-3/validation-1.0.3.md).
The 1.0.3 candidate is unreleased as a stable tag. Its
[program-only devnet deployment](docs/deployment/DEVNET_1_0_3_DEPLOYMENT_2026_10_22.md)
is complete; do not infer initialization, functional market readiness, or
mainnet release approval from that deployment.
The final candidate also includes the AI3-FR01 quarantine and the owner-selected
complete SHA-256 VenueGuard correction for first deployment.

## 1.0.3 release blockers

- [ ] Complete or explicitly disposition the [new audit checkpoint](docs/audit/2026-10-22-progress/README.md)
      before any whole-codebase audit-complete claim. Seven independently retained
      hypotheses require validation and 23 lack independent disposition; preserve
      the two provider-stopped scope gaps without retrying those tasks.
- [ ] Verify actual funded/monitored keeper operation, covered markets, payer top-ups and
      alerts, plus running indexer destination `3` and 304-byte client artifact identities.
      [Local service staging](docs/deployment/devnet-local-service-staging-2026-10-22.md)
      established source-matched builds, PostgreSQL migrations and a bounded
      deployment-only backfill and brief live connectivity/finality smoke with
      a loopback reader; sustained ingest with market events, funded keeper and
      alert delivery remain pending.
- [ ] Complete the objective gates and final exact-revision checks in the
      [1.0.3 tag/deployment plan](docs/audit/release-checks-1.0.3-2026-10-22.md)
      before any stable tag or mainnet deployment. The
      [owner-delegated decision](docs/audit/owner-delegated-release-decision-1.0.3-2026-10-22.md)
      is recorded as **no-go now**; no further generic policy choice is pending.
      The canonical runtime bytes and prior ≥24 h fuzz attribution are established
      for `7b11cc7` by fresh native/pinned-Docker builds and source-input equivalence.

The dated [Jupiter mainnet pin disposition](docs/audit/probes/release-wrap-2026-10-22/jupiter-mainnet-pin-2026-10-22.md)
closed the observed configuration drift with a passing fresh live check. Recheck
the live identities at the final release revision; this does not add a Jupiter
trade adapter or remove the remaining operating and audit gates.

## Public-devnet test prerequisites

- [ ] Resolve the [devnet market feasibility gates](docs/deployment/devnet-market-feasibility-2026-10-22.md)
      before promising market E2E: canonical Raydium/Jupiter IDs are not
      executable there, devnet Squads fails the supported immutability check,
      and current Pump/PumpSwap/fees executable bytes differ from the pinned
      mainnet fixtures. A [bounded dispatch and published-interface probe](docs/deployment/devnet-pump-interface-probe-2026-10-22.md)
      passed, but full account-populated CPI compatibility remains unproven. A fresh Pyth
      sample passed at the dated read, but must pass again at execution. Confirm
      a real market, governance member identities and installation of the
      [staged coverage key](docs/deployment/devnet-operator-plan-2026-10-22.md#selected-topology-and-identities),
      and funded live operators;
      the program-only deployment and local service build do not close these.

The [owner-selected policies](docs/audit/owner-decisions-1.0.3-2026-10-21.md) and
accurately labelled [focused](docs/audit/independent-ai-fix-review-1.0.3-2026-10-21.md) /
[integration](docs/audit/independent-integration-review-1.0.3-2026-10-21.md) /
[AIR2](docs/audit/independent-air2-fix-review-1.0.3-2026-10-21.md) AI fix reviews
are recorded. The first program-only devnet deployment is complete; no old
accounts required migration for that deployment.
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
- [ ] Measure Jupiter's ProgramData extension hash compute/latency on the
      reviewed binary before adding Jupiter to a future venue latch mask. The
      current ProgramData/ELF sizes and hashes are already recorded in the
      [dated mainnet pin review](docs/audit/probes/release-wrap-2026-10-22/jupiter-mainnet-pin-2026-10-22.md).
- [ ] Optional: make the documented keeper deployment/alert thresholds
      configurable.
- [ ] Repair or re-record the changed-message signature in
      `raydium_fast_path_venue_review.jsonl` before treating it as signed
      INGEST evidence.
- [ ] If using the drained-pool fuzz figures to claim a risk reduction,
      measure a comparable pre-AIR baseline; the current 10-minute run alone
      does not establish that comparison.
