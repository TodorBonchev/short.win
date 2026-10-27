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
is complete. Since that deployment, GlobalConfig and two Pump markets with
lender deposits were initialized. A supervised short open, close and restored
market pause were finalized on the deeper market; see the
[trading receipt](docs/deployment/devnet-trading-bootstrap-2026-10-23.md).
The [keeper, indexer and devnet website](docs/deployment/devnet-web-publish-2026-10-23.md)
are running. These bounded devnet results do not establish unattended trading
readiness. The owner subsequently selected the mainnet preparation policy below.
The final candidate also includes the AI3-FR01 quarantine and the owner-selected
complete SHA-256 VenueGuard correction for first deployment.

## Jupiter redesign before first mainnet

The owner postponed first mainnet on 2026-10-27 for broad Jupiter support. Follow
the [recorded decision](docs/audit/owner-jupiter-redesign-decision-2026-10-27.md) and
[design proposal](docs/design/JUPITER_REDESIGN.md). The owner selected
signed Jupiter-derived risk feeds, evaluating Switchboard first, with concrete
trust/settings reviewed before deployment. Unchecked keeper JSON remains forbidden;
new tokens stay disabled until feed/size/liquidation requirements qualify.

- [ ] Specify and validate the selected signed-feed pricing, quantity/depth and failure rules.
      Native component ABI checks passed, but the candidate SDK verifier is
      [quarantined for source-level safety issues](docs/audit/jupiter-oracle-sdk-review-2026-10-27.md).
      Complete the [exact-quantity producer contract](docs/design/jupiter/QUOTE_PRODUCER_CONTRACT.md)
      and separately authenticated lower-reference policy; upper-only routes cannot force liquidation.
- [ ] Implement bounded Jupiter execution, risk integration and compatible services.
- [ ] Independently review and qualify final code/fixtures, then run fresh fuzz.
- [ ] Select the new release version and complete funded mainnet commissioning.

The old Token-2022-only 24h campaign is unstarted/onhold; retain its actual
CI/bench evidence. Do not spend a new 24h qualifying the superseded launch scope.

## Preserved 1.0.3 mainnet preparation baseline

The owner selected **mainnet before an external audit and a funded bug bounty**,
funded later from protocol fees, and explicitly accepted **existing per-market
borrowing caps with uncapped deposits and market count**. The compiled default
opening-notional cap is 25 SOL per market, not a global TVL or loss cap. See the
[current release plan](docs/release/MAINNET-1.0.3.md) and
[dated decision](docs/audit/owner-mainnet-decision-1.0.3-2026-10-27.md).

- [ ] After the selected Jupiter redesign is implemented, qualify its final release
      inputs and repeat current live identities before selecting the new final version.
      The [Token-2022 engineering follow-up](docs/audit/token2022-qualification-1.0.3-2026-10-27.md)
      is complete: repaired PR CI 40443684567 passed 24 jobs with two intentional
      skips; source-equivalent dispatch 40440067371 killed all 19 selected mutants
      and failed only its subsequently repaired browser test. This combines two
      accurately scoped runs, not one 25-job repaired manual run.
      The fresh all-profile 24-hour campaign is **NOT STARTED/on hold** because
      the owner postponed mainnet for the Jupiter redesign decision. No long-run
      host choice is required for this pre-redesign baseline. Changed redesign
      inputs need their own qualifying evidence; a source tag proves no funded operation.
- [ ] Commission the [VPS operator stack](docs/deployment/MAINNET_OPERATIONS.md):
      actual mainnet RPC, covered markets, payer funding/top-ups, active keeper
      coverage and oracle, independently delivered alerts, private database,
      off-host backup/restore, and exact indexer/client artifact identities.
      Devnet's paused/empty idle profile is unsuitable for active mainnet.
- [ ] Supply real mainnet governance/emergency public identities, derived treasury,
      coverage authority, program/deployment payer and exact configuration. Prepare
      and simulate a bounded mainnet transaction packet before signing; no key or
      concrete financial transaction is supplied by the policy decision alone.
- [ ] Verify current live dependencies, deploy canonical bytes, verify actual chain
      state and commission the keeper/readers before enabling new shorts. Disclose
      unaudited status, current bounty status and uncapped deposits/market count.

External audit/contest and funded bounty are **post-launch funding work**, not
prelaunch requirements. Obtain quotes/targets and account for earned protocol-fee
security reserves without diverting lender funds or exit operating reserves.
The [new audit checkpoint](docs/audit/2026-10-22-progress/README.md) remains
incomplete: seven hypotheses need validation, 23 independent dispositions remain,
and blocked/stopped scopes remain disclosed. No audit-complete claim follows
from the owner's timing waiver; preserve provider-stopped scopes without retrying.

The [Jupiter pin disposition](docs/audit/probes/release-wrap-2026-10-22/jupiter-mainnet-pin-2026-10-22.md)
closed its dated configuration drift. Repeat the full live check at release time;
this does not enable a Jupiter trade adapter. The
[October 23 assessment](docs/audit/release-gate-assessment-1.0.3-2026-10-23.md)
and [earlier delegated no-go](docs/audit/owner-delegated-release-decision-1.0.3-2026-10-22.md)
are historical timing decisions superseded by the owner's October 27 instruction.

## Public-devnet test prerequisites

- [ ] Resolve the remaining
      [devnet market feasibility gates](docs/deployment/devnet-market-feasibility-2026-10-22.md)
      before promising broader market E2E: canonical Raydium/Jupiter IDs are not
      executable there, devnet Squads fails the supported immutability check,
      and current Pump/PumpSwap/fees executable bytes differ from the pinned
      mainnet fixtures. A bounded account-populated Pump open/close now passed
      on one deeper devnet market; it does not prove graduation,
      liquidation, fallback settlement, other venues or sustained oracle
      coverage. The newly created controlled Pump coin remains too shallow to
      admit a legal short under the deployed limits.

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
