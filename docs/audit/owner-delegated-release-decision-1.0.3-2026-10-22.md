# Owner-delegated 1.0.3 release decision — 2026-10-22

> **2026-10-27 policy update:** The owner’s direct October 27 decision supersedes the delegated no-go and audit-first release timing below. Devnet approvals/receipts retain their original scope. See the
> [current mainnet plan](../release/MAINNET-1.0.3.md) and
> [owner decision](owner-mainnet-decision-1.0.3-2026-10-27.md).

The owner instructed the agent to perform remaining owner-side work, choose the
recommended answers and mark delegated decisions complete. This records those
choices under that authorization. It does not attribute implementation or
verification to the owner and does not convert missing evidence into a pass.
The earlier [economic and fix-review policy choices](owner-decisions-1.0.3-2026-10-21.md)
remain in force; they do not need to be signed off again.

| Owner-side choice | Delegated decision |
|---|---|
| Release call now | **No stable `v1.0.3` tag or mainnet deployment yet.** Continue the bounded, uninitialized devnet rehearsal and local operator setup. Reassess automatically when the objective gates below have evidence; no further generic policy-choice question is pending. |
| Main integration | Honor the earlier instruction for `main` to contain the latest work by fast-forwarding the release-preparation branch after its local checks. Keep `v1.0.3` untagged and the no-go release call intact; branch integration alone does not certify the broader audit or live operation. |
| Version | Keep the unreleased candidate at **1.0.3**. The devnet program matches its canonical ELF. Do not call identical bytes 1.0.4. A change to the release artifact invalidates the current exact-byte qualification and needs fresh CI/fuzz attribution before tagging; choose the version deliberately. If 1.0.3 has already shipped elsewhere, use a later patch version. |
| Devnet authority and scope | Retain the payer as temporary upgrade authority while the program is uninitialized. Do not use mutable devnet Squads as if it passed the compiled immutability check. Keep Raydium/Jupiter and ordinary users out of the first market test; a later Pump/PumpSwap smoke requires real compatible accounts and an independently reviewed transaction packet. |
| Operator defaults | Use the [local devnet operator plan](../deployment/devnet-operator-plan-2026-10-22.md): one loopback writer/reader and one market at a time, distinct keeper and coverage identities, a 2-SOL keeper target with review below 1.5 SOL, and funded zero-reward exits. The owner is the default local rehearsal operator and escalation owner until a separate reachable alert recipient and actual authority holder are named and tested. This choice is not evidence that a keeper or pager is running. |
| Jupiter drift | **Administrative configuration disposition completed.** The [fresh mainnet review](probes/release-wrap-2026-10-22/jupiter-mainnet-pin-2026-10-22.md) pinned the current ProgramData/ELF/IDL and refreshed the six-slot example; the aggregate live check now passes. Preserve the older failed log, keep Jupiter outside the current trade profile, and repeat the live check at final release time. |
| Audit claims | The earlier focused fix reviews and final-byte fuzz are accepted with their exact scope. The newer whole-repository checkpoint remains **incomplete**; do not claim a full or external audit. Preserve the two provider-stopped scope gaps without retrying or reassigning them. |
| Optional media and research | Corrected article text is prepared for owner publication. Video upload and conditional measurements are later work, not prerequisites for the devnet loader deployment or permission to misstate release readiness. |

The [program-only deployment](../deployment/DEVNET_1_0_3_DEPLOYMENT_2026_10_22.md)
is finalized and byte-verified. The remaining objective gates are: disposition
the broader audit hypotheses without misreporting stopped scopes; establish actual funded,
monitored keeper/indexer/client operation; and prove a compatible initialized
devnet market before claiming market readiness. The [pending-work list](../../TODO.md)
tracks these separately. Passing one does not imply the others passed.

This delegation is a release-policy decision, not a blanket signature for a
new on-chain transaction. The applied `solana-dev` skill's W009 transaction rule
requires a concrete cluster/payer/recipient/amount/fee summary, simulation and
explicit approval before any later signing. The previous approval was for the
program-only loader deployment and is already spent.
