# Jupiter redesign implementation milestones

The owner selected signed Jupiter-derived risk observations and then explicitly
selected an in-house price producer, keeper and program implementation. Use a
simple initial host and ordinary Solana reporter transaction authorization.
Reporter keys and concrete financial/failure settings need review before deployment.
Mainnet is postponed; the working branch name 1.1 remains provisional.

| Phase | Work | Current status |
|---|---|---|
| 0 | Record architecture, operator-price trust direction and mathematical contracts. | In-house direction selected; exact production settings/keys are not approved. |
| 1 | Pure V2 header/fixed-meta parser and exact-quantity cost math. | Implemented unwired prototypes; focused tests and independent review retained. |
| 2 | Build the in-house fixed-q producer and authorized report/update contract; measure remote quote/simulation workload. | Producer contract and deployment shape drafted. No production worker/update instruction or live report is implemented. |
| 3 | Isolated bounded execution SBF probe on pinned Jupiter, hostile account/route tests and measured limits. | Not implemented; header parsing is not routed execution. |
| 4 | Program report/risk state, history/admission/failure rules and keeper/client/indexer integration. | Not implemented. New markets stay disabled until qualified. |
| 5 | Full source/ABI/fork/regression/CU verification and independent integration review; freeze runtime and fresh qualifying fuzz. | Prototype full CI passed 25 jobs on its recorded source. Final integration/fuzz remains open; old duration does not qualify new inputs. |
| 6 | Review exact reporter permissions, risk terms, API/RPC costs and funded operations; choose version/tag/transaction packet. | Pending concrete review. No deployment or financial execution. |

The [in-house design](INHOUSE_PRICING_AND_LIQUIDATION.md) uses ordinary Solana
transaction signatures and a scoped on-chain reporter allowlist. This removes a
separate oracle vendor, not operator-price trust or Jupiter/RPC dependencies.
The proposed price worker shares the keeper codebase but runs under separate
supervision so source requests do not stall exit execution. There is no new CLI yet.

The [fixed-q producer contract](QUOTE_PRODUCER_CONTRACT.md) still applies. A candidate
ask upper is not a financial lower reference for forced decisions. The pure math
requires exact supported position quantities and distinct finite/unknown/no-route
statuses; a larger grid node's upper mark cannot become an exact-q liquidation lower
edge. Financial history belongs in program state, not an arbitrary indexer snapshot.

The route prototype checks only header scalars and fixed metas. Remaining accounts,
actual privileges, route behavior, executable/mint identity, scratch authority,
net spend/output and CPI isolation remain authorization work. No existing handler
accepts the prototype as route authorization or admits a new asset.

The [initial-host plan](../../deployment/MAINNET_OPERATIONS.md#in-house-price-worker-on-the-initial-host)
is an engineering estimate requiring combined-load measurements. Keep report roles,
source/policy/quantity binding, freshness, sequence and completed-epoch history,
outage repayment and admission gates explicit before production.

## Preserved candidate research

Switchboard was evaluated first, then excluded after its shutdown announcement.
Its standalone native ABI/format/refusal results and source-level SDK safety findings
remain research receipts, not the selected runtime or a dependency to repair/promote
for this in-house design. No production service/program links that SDK. The
[review](../../audit/jupiter-oracle-sdk-review-2026-10-27.md) and
[candidate screening worksheet](PROVIDER_TRUST_REVIEW.md) preserve that history.
The RedStone capability request is an unused, unsent draft. No third-party outreach,
provider contract, new signer authority or payment has been performed.
