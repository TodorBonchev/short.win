# Owner decision: postpone mainnet for Jupiter redesign — 2026-10-27

The owner explicitly answered the release-scope question:

> Postpone the first mainnet launch and redesign for broad Jupiter support now

This supersedes the immediate first-mainnet timing of the earlier October 27
[release decision](owner-mainnet-decision-1.0.3-2026-10-27.md). The desired design
retains direct Pump bonding-curve support and broadens other execution through
Jupiter, including eligible assets routed through supported DEXes. The existing
per-market borrowing cap and mint/lender protections remain the baseline; no
new global cap or universal token/exit guarantee is asserted.

The owner then asked whether Jupiter quotes can supply margin/liquidation
pricing. After the distinction between raw API responses and verified oracle
feeds was explained, the owner explicitly selected:

> Proceed with signed Jupiter-derived feeds and review the concrete trust/settings before deployment (Recommended for broad coverage)

This selects a signed quote-oracle architecture for new markets and authorizes
implementation/prototyping. Evaluate Switchboard first; a provider, live feed,
authority/queue/quorum, pricing/cap/tier/settlement settings or financial
transaction is not thereby approved. Keep mint/borrowing protections and leave
new markets disabled until their feed, trade-size pricing and liquidation rules
qualify. Review concrete trust and settings before deployment. The
[design proposal](../design/JUPITER_REDESIGN.md) separates execution proposals
from authenticated risk observations. Unchecked keeper/API JSON remains forbidden.

The source branch is provisionally named `codex/jupiter-redesign-1.1`; this is a
development name, not an approved final version or tag. The current 1.0.3 work
remains preserved as baseline evidence. Its old completed fuzz duration cannot
qualify the new program, and the unstarted Token-2022-only long campaign remains
on hold while the new program scope is determined. Continue retaining actual
CI/fork/benchmark results without calling the new design qualified.

The intended initial-mainnet sequence remains **before an external audit and
before a funded bounty**, funded later from earned protocol fees as previously
recorded. This timing change introduces no audit-first requirement, changes no
fee allocation and claims no external-firm certification. Final runtime tests,
reproducible bytes, independent review, fresh qualifying fuzz, current upstream
compatibility and real funded/monitored operation are still required.

No program rewrite, source tag, deployment, financial transaction, oracle feed
creation, new signer authority or long fuzz run is claimed by this record.

## Later owner selection: in-house pricing and a simple initial host

After the operator-price trust distinction was explained, the owner selected:

> Lets go with the in house and keep things simpler. Would the price service fit in the recommended vps

The implementation direction is now a short.win-operated price producer and
authorized on-chain report updates, with our own keeper orchestrating atomic
Jupiter swaps. A separate oracle company and hosted Jupiter Trigger custody are
not selected. The RedStone capability inquiry remains unsent and is closed as an
unused draft; no outreach or financial commitment was authorized or made.

Use ordinary Solana transaction signatures and scoped reporter authorization for
the initial design, rather than introducing a third-party oracle SDK or detached
signature framework. Keep the price worker in the keeper's Rust codebase with
separate supervision/reporting credentials, bounded work and shared clients/types.
One initial VPS is an operational simplicity choice, not independent quorum or
high availability. Its price-reporting authority is a disclosed financial trust
role even though it cannot directly withdraw protocol vault funds.

The recommended 4 dedicated vCPU / 16 GiB host is a starting engineering estimate
for remote Jupiter quotes and remote RPC simulation, not measured capacity. It
does not run a full validator or local Jupiter routing engine. Measure the actual
combined workload and request budgets before activation. Exact reporter keys,
financial/reference/failure parameters, production service limits and deployment
still require their concrete review and qualification. First mainnet remains
postponed; the existing mint, lender and borrowing protections remain required.
