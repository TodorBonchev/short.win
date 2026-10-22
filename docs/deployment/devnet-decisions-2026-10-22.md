# Selected devnet decisions — 2026-10-22

The owner instructed the agent to make the best project decisions and complete items where feasible. The selections below are complete under that delegation; they need no further generic policy approval. Actual execution and evidence remain distinct. This document adds no new approval gate and does not claim a deployment, initialization, running service or passing check.

## Completed decisions

| Decision | Selected disposition |
|---|---|
| First phase | **Deploy only shortwin, leaving it uninitialized.** Verify the loader, ProgramData, authority and executable identity. Initialization and markets are later phases. This loader-only rehearsal needs no running keeper/indexer, trading frontend, live venue, fresh market oracle or verified governance topology. |
| Version and tag | Keep the existing unreleased **1.0.3** package identity. Create no tag for this preparation. Any later version/tag must accurately match its embedded metadata and evidence. |
| Program identity | Retain `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK` and its existing matching program signer. Keep the program signer separate from the payer. |
| Initial upgrade authority | Use existing devnet payer `3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14` as the temporary initial rehearsal upgrade authority. Keep the program upgradeable and record that custody explicitly; it establishes no production governance assurance. |
| Governance | Defer initialization and Squads transactions. Preserve the existing immutability requirement: mutable devnet Squads cannot pass it or prove the production governance topology. Select a compatible or explicitly documented devnet authority layout before initialization. |
| Venue scope | Exclude Raydium and Jupiter from this phase. Preserve fixed canonical mainnet IDs. Official devnet Raydium IDs require a separate reviewed build profile. Prefer bounded Pump/PumpSwap smoke later, conditional on actual dependency and market compatibility. |
| Oracle and guards | Preserve the 120-second oracle maximum age, complete 32-byte fingerprints, 304-byte VenueGuard and latch rules. Before market testing, use fresh observations and reviewed devnet slots/extras; never substitute invented or stale mainnet slots. |
| Allocation and budget | Prefer max length equal to the reviewed ELF length. Set a **20-devnet-SOL deployment ceiling**, including concurrent buffer/program funding and fees, preserving at least 5 SOL of the documented 25-SOL balance. Recalculate from the actual artifact/CLI packet. An estimate exceeding the ceiling requires a revised concrete budget. |
| Frontend and operators | Defer trading publication and live services to the initialized/market phases. Use the [operator plan](devnet-operator-plan-2026-10-22.md) for distinct service keys, funded zero-reward exits, monitoring, indexer destination `3` and compatible clients. These requirements remain real-market and production gates. |
| Testing | Deploy only the reviewed shortwin artifact. Keep the TEST-ONLY venue, mock CPAMM and synthetic Playwright fixtures local; local/fork results are not live-devnet evidence. Preserve production release checks and the stopped review scopes. |

## Completed evidence and work still pending

The [readiness record](DEVNET_1_0_3_READINESS_2026_10_22.md) documents a matching canonical program-signer public key and a **25-SOL** payer balance at 2026-10-22 07:20:41 UTC, closing signer-identity selection and the 20-SOL planning funding target. These are dated observations. Refresh balance and cluster/genesis for the concrete execution packet; no deployment follows merely from funding.

Artifact equivalence is now established by the [fresh native/pinned-Docker builds](../audit/probes/release-wrap-2026-10-22/byte-rebuild/SUMMARY.md) of **`7b11cc7`**: canonical release/test/mock bytes match, and fuzzer/harness/fixture inputs are unchanged. The user-linked completed ≥24-hour record applies to those runtime inputs with its original dates and coverage, not to changed services. Docker access is repaired. No new fuzz run is claimed; provider-stopped audit/fuzz work remains stopped.

The [loader-only packet](../audit/probes/release-wrap-2026-10-22/deployment-packet/deployment-packet.md) records the exact source/artifact, real buffer and signer identities, devnet genesis/RPC, allocation, balance/costs and successful unsigned buffer preparation. The first explicitly approved attempt signed and confirmed buffer creation and 14 writes, then stopped with a CLI retry error; the program is absent. A separate concrete transaction review is needed before any recovery upload or buffer close. Retain actual signatures/slot and post-deployment loader, ProgramData, authority and executable-hash checks if deployment later completes; this preparation does not claim them.

Initialization and real-market testing still need the supported authority layout, scoped Pump dependency/market evidence, fresh Pyth, guard/extras, market/ALT/capital selections and actual funded/monitored keeper, compatible indexer/readers and UI behavior. The canonical Raydium/Jupiter absence and mutable Squads mismatch are not resolved by a successful loader rehearsal.

The market-E2E prerequisite and incomplete release blockers in [TODO](../../TODO.md) therefore remain pending. Stable release approval, mainnet Jupiter live-pin disposition, broader audit claims and operating evidence are separate from this devnet loader phase. Scope and policy selection are complete; unverified implementation, execution and evidence must not be checked off.

Basis: [devnet readiness](DEVNET_1_0_3_READINESS_2026_10_22.md), [owner-selected production policies](../audit/owner-decisions-1.0.3-2026-10-21.md), current owner delegation and root's reported build status. This documentation task performed no source review, build, test, network request, signing or deployment.
