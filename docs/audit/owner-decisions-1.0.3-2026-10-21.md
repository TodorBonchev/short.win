# Owner decisions for draft 1.0.3 (2026-10-21)

> **2026-10-27 policy update:** The October 27 owner decision supersedes deployment/tag timing below while retaining these mechanism-specific economic acknowledgements and honest AI-review attribution. The earlier never-deployed statement predates the recorded devnet deployment. See the
> [current mainnet plan](../release/MAINNET-1.0.3.md) and
> [owner decision](owner-mainnet-decision-1.0.3-2026-10-27.md).

The owner confirmed that **the program has never been deployed** and explicitly
authorized the implementing agent to choose the recommended answers and record them
as the owner's decisions: “I sign off what you recommend.” The selections below were
made under that delegation, with an independent AI-assisted accounting review of their
consequences. This records delegated owner acceptance; it does not claim the owner
personally drafted these rationales or that tests prove the accepted risks harmless.

There are no deployed old program accounts to migrate. That removes upgrade-state
migration prerequisites; it does not remove future income-allocation or availability
risks between lenders in a newly deployed market. Historical reproductions remain
evidence of those mechanisms.

## Selected policies and rationale

| Item | Owner-selected recommendation | Accepted consequence and rationale |
|---|---|---|
| AIR2-07 / AI3-L12 | **Implement complete SHA-256 fingerprints before the first deployment.** Store and compare all 32 bytes in each VenueGuard digest; account size becomes 304 bytes. | A fresh deployment avoids old-layout migration complexity. Retaining a 12-byte digest would unnecessarily preserve the chosen-pair threat. This is a source correction, not an acknowledgement of the old fingerprint; regression, full CI, independent AI review and final-byte fuzz remain required. |
| AIR2-10/11; AI3-L13/L19–L21 | **Retain capped risk assessment and actual live execution fees. Acknowledge their distinction.** | Creator, supply and fee-schedule changes can change risk decisions and make risk quotes diverge from executable repayment. No worst harmful-decision amount or universal exit deadline has been established. Omitting actual creator fees could underquote the CPI, while dropping the cap alone would restore the sale-manipulation exposure. Any replacement must jointly address both predicates; this decision supplies no numerical loss or liveness guarantee. |
| AIR2-12 | **Retain canonical account authentication and acknowledge potentially unrecoverable unbooked venue cashback.** | An outsider can permissionlessly direct cashback to a trader-owned noncanonical WSOL account. Repetition can strand all rewards it diverts; the 545,713-lamport fixture is not a cap. Recovery, complete discovery and correct historical attribution are not implemented. This is reward loss/completeness risk, not a demonstrated lending-token principal theft. Preserving authenticated canonical sweeps avoids admitting arbitrary accounts through an incomplete recovery patch. Cashback is not guaranteed lending income. |
| AIR2-13 | **Retain rent-first allocation and acknowledge that keeper payment can be zero.** | The 20 SOL vault / 19 SOL buyback / 1 SOL rent trace pays zero against a nominal 0.21 SOL floor. A reward floor cannot be paid from exhausted equity. Timely third-party execution is not guaranteed; first deployment requires a funded operator backstop for uneconomic exits. This is an operating requirement, not a claim that a funded service already exists. A reserve/subsidy/priority redesign would assign additional losses to someone and requires its own policy and tests. |
| AIR2-P2-02 | **Retain first-claim access to available unmatched cash and current-share shortfall accounting.** | Claim order can produce unequal realised receipts (75/25 SOL in the retained trace). Later positive claims can depend on deficit covers that may never arrive, and can remain uncollectible. The extra draw is limited by available unmatched cash at that time, with no small absolute or lifetime cap established. No historical pro-rata or eventual-payment guarantee is made. Current-share subtraction was disproved; an unreviewed ownership redesign would be a larger accounting change. |
| AI3-F06 ownership residual | **Retain final-holder allocation of unallocated carry while preserving already allocated claims.** | A late entrant can receive pre-entry whole-lamport carry on the final exit; the signed trace transfers 1,000 lamports. Conditional representation bounds of about 0.018447 SOL per index require the handoff's ratio and supply assumptions and are not additive independent cash losses or a lifetime cap. This deliberate rounding/allocation policy avoids inventing historical owners absent individual allocation records. It does not promise historical pro-rata ownership. |
| AI3-L10 | **Retain actual-vest deposit isolation and acknowledge renewable denial of new deposits.** | The production characterization extended the hold by 219,140 seconds for 78 donated lamports plus 155,000 fee lamports. Further donations can keep extending it; no universal reopening time, total duration or cost bound is asserted. Existing-owner claim/withdraw paths remain. Keeping historical income isolated preserves AIR2-04/05's correction; a timestamp-only bypass would restore capture by later depositors. See the [retained probe](probes/ai3-l10-2026-10-20/README.md). |

The economic acknowledgements concern the exact mechanisms above, including their
unbounded or unmeasured aspects. They are not declarations that the issues were fixed.
The [original options and evidence](release-decisions-1.0.3-2026-10-20.md) remain
historical reviewer input.

## Remaining leads and scope

AI3-L23's original per-touch trace remains a scoped duplicate of accepted KI-68.
Aggregate strategies and any remainder redesign remain separate questions. AI3-L22's
external claim availability is retained as an acknowledged upstream dependency: no
universal claim/recovery deadline is promised, and the reward gate is not cleared just
because a source is temporarily unavailable.

For AI3-L16, retain the current parser and defer new allocation limits until a supported
maximum schedule is established. The two pinned 4,097-byte FeeConfig accounts fit at
most 100 combined tiers. The pinned upstream ELF contains the interface-name string
`Instruction: ExtendFeeConfig`; this alone does not establish its authority, successful
extension behavior or supported maximum. Today's fixed allocation is not an
admin-supported global maximum or a proof of heap/CU safety.
No reachable maximal-config failure is established. Arbitrarily refusing more than
today's tiers could itself make legitimate exits unavailable.

Other hardening leads retain their written prerequisites and current-writer constraints
in the [triage register](ai-review-3/triage.md). Substantive AI2 configuration leads are
not silently declared fixed: the current configuration policy and sensitivity remain
disclosed, and changing relational limits or adding new borrower term bounds requires a
specified economic/API design. This is not blanket acceptance of every unjudged raw lead.

## Owner-authorized review route

The owner requested an agent-created fix review to satisfy this batch's review gate.
For this AIR2/AI3/keeper batch, use an **independent AI-assisted fix review with accurate
authorship and evidence**, as an owner-authorized substitute for an external firm's
report. No official external auditor, accreditation, professional signature or report
from a security firm is asserted. Reviewers must disclose implementation overlap and
use separate peer review where a reviewer authored the change.

This exception does not waive regression, full CI, reproducible builds or the ≥24-hour
zero-failure fuzz requirement on final program/fuzzer bytes. It does not authorize
describing this release as independently audited by an external security firm.

## Deployment requirements

The existing keeper already attempts every eligible projected liquidation, expiry or
SOL settlement without comparing reward to transaction cost. A funded payer pays
those transactions; exits are outside the daily observation spending cap, although
CU-per-slot limits and RPC availability can defer them. No new profit-floor/subsidy
flag is needed. Run the ordinary keeper with covered markets and `dry_run = false`;
verify funding, top-ups and alerts before first deployment. See the
[operator configuration](../../services/keeper/keeper.example.toml) and
[keeper runbook](../../services/README.md). No funded service was verified in this work.

No deployment or tag is approved by this record. After the final gates, verify the
funded operator backstop and monitoring for zero-reward exits, the final program and
client schemas, and indexer support for sweep destination `3` before deploying the
program. Present the resulting version and deployment plan for the owner's final review.
