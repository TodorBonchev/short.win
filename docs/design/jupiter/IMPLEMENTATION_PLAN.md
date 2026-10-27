# Jupiter redesign implementation milestones

The owner selected signed Jupiter-derived risk feeds for new markets, evaluating
Switchboard first and reviewing concrete trust/settings before deployment. Mainnet
is postponed. The working branch name 1.1 is provisional; no final tag exists.

| Phase | Work | Current status |
|---|---|---|
| 0 | Record scope, architecture alternatives, selected trust direction and mathematical contracts. | Drafts and decision recorded; provider/settings not approved. |
| 1 | Pure Jupiter V2 header/fixed-account parser and typed finite quantity-cost math. | Implemented prototypes; no production dependency or instruction wiring. Local focused checks and independent review retained. |
| 2 | Verify canonical fixed quote-job identity; compile actual oracle verifier/type bridge; obtain authentic signed updates and resource measurements. | Offline identity spike and pinned native ABI probe passed. Switchboard excluded after its shutdown announcement; SDK verifier quarantined after independent source-level safety findings. Safe format/refusal checks passed24 native tests. No live feed, repaired verifier or signature/SBF proof yet. |
| 3 | Isolated bounded execution SBF probe on actual pinned Jupiter, with hostile account/route tests and measured limits. | Not implemented; parser tests are not a routed swap. |
| 4 | On-chain provider/risk state, history/admission/failure rules, keeper and client/indexer integration; enable only qualified markets. | Not implemented. Ordinary API JSON never supplies authoritative prices. |
| 5 | Full source/ABI/fork/regression/CU verification and independent integration review; freeze final runtime inputs and fresh qualifying fuzz. | Not started. Prior 1.0.3 duration does not qualify new code. |
| 6 | Review provider keys/queue/quorum/feed IDs, risk terms/costs and funded operations; choose version/tag and actual transaction packet. | Pending concrete review. No deployment or financial execution. |

The mathematical prototype requires exact supported quantities and treats unknown,
stale and attested-no-route as distinct from finite cost. A larger grid node's
upper mark is not reused as an exact-size forced-liquidation lower bound.
Endpoint-only fair-value checks are insufficient where fee branches change.

The route prototype certifies only header scalars and fixed account/meta bindings.
The opaque route plan, arbitrary writable accounts, executable program identity,
PDA/mint authority, actual spend/output and CPI isolation remain unimplemented
authorization work. No current handler accepts this type or admits a new asset.

New-market oracle configuration must fail closed until the verifier, immutable
feed identity, units/quantities, freshness/epoch coherence and financial policy
are established. The initial actor/provider/settings remain reviewable even
though the owner authorized the architecture and implementation.

The next producer step follows the [fixed-quantity contract](QUOTE_PRODUCER_CONTRACT.md).
Fixed-input outAmount jobs do not yet supply B_hat(q), and a candidate ask upper
cannot supply an economic lower reference for forced actions. The candidate SDK's
[native pass and qualification blockers](../../audit/jupiter-oracle-sdk-review-2026-10-27.md)
are separate evidence. Keep its verifier disabled pending reviewed repair/runtime proof.

The [candidate provider trust worksheet](PROVIDER_TRUST_REVIEW.md) records observed
program/queue/authority identities and the evidence needed for concrete settings
review. It approves no live provider, quorum, age bound, financial policy or spend.

Switchboard evaluation is retained as research after the provider's shutdown notice.
Evaluate a maintained report provider or an explicitly reviewed attester network;
no alternative provider, signer authority or business commitment is yet selected.
The capability request is prepared and unsent. This changes no selected financial
risk settings and does not restart or qualify the old long campaign.
