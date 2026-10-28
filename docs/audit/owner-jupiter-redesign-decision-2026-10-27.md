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

## Later delegation: choose the simplest liquidation model

After discussing hosted Jupiter limit orders, partial fills and repayment in a
mixture of tokens and SOL, the owner instructed:

> Ok decide what you think will be best for the project and simplest

Under that delegation, select whole-position atomic buyback/repayment with the
existing actual-cost waterfall as the implementation baseline. Use bounded,
isolated protocol-funded Jupiter execution, program-authorized eligibility and
actual net receipt of all `q` debt tokens. An underfilled/invalid attempt rolls
back; keeper retries must use fresh evidence within existing limits and budgets.
Do not introduce hosted Trigger custody, asynchronous liquidation orders or
partial/mixed token-SOL repayment in the first release. Internal route splitting
is compatible with the whole-debt atomic postcondition.

A genuine collateral gap can leave lenders unable to replace all owed tokens.
Cash/default settlement remains a distinct path, with explicit eligibility,
valuation, persistence and loss accounting; quote/swap failure alone cannot
activate it. The existing S1/S2/S3 rules require review and qualification for the
new reference backend. Admit no new market until its normal exit and recovery
policy are specified and tested. This engineering selection adds no numerical
risk/settlement terms, deployment authorization or principal guarantee. See the
[selected liquidation contract](../design/jupiter/INHOUSE_PRICING_AND_LIQUIDATION.md#first-release-liquidation-choice).

## Later owner selection: trusted off-chain history and on-demand updates

After rejecting the continuous on-chain publication cost, the owner instructed:

> I am ok my vps monitor price history off chain and our service to be trusted. Allign all documents and code with this and do the changes required so we are ready for launch

This explicitly authorizes the service to retain and attest historical prices off
chain. Use a distinct trusted-history snapshot model, ordinary allowed reporter
transaction signatures, and fresh on-demand cache updates for actual actions.
There is no periodic on-chain heartbeat requirement in this model. Preserve the
older public-history implementation and cost measurements as development evidence;
they are not the active publication design or a recurring SOL spending commitment.

The program can check signer authorization, immutable domain/quantity/policy,
sequence, timestamps, numerical bounds and its own elapsed-time rules. It cannot
prove that privately attested samples existed at the claimed times or that the
operator omitted no gaps. A compromised or mistaken reporter can manufacture
history and cause false financial decisions. One VPS remains one trust and
availability domain. The indexer archive aids diagnosis; it does not remove that
trust or become a second independent oracle.

Keep whole-position atomic Jupiter repayment, price-independent borrower repayment
and bounded voluntary exits. Fresh matching snapshots may be reused within their
reviewed lifetime. Keeper update/action batching needs packet/CU qualification;
separate refresh and action transactions are not jointly atomic. Refreshing the
cache after wallet approval must not modify already-signed action bytes, and
stale evidence, changed state or violated borrower bounds must refuse execution.

Sparse on-chain arming/rechecking may prove two separated checks, not continuous
unhealthy pricing between them. Privately claimed history alone must not create
an immediate collateral-confiscation path. At the time of the history selection, overdue collateral default and healthy-
borrower forfeiture were a separate unanswered choice. The subsequent explicit
selection below resolves that choice; history trust alone did not select it. Concrete keys,
policy settings, final tests/CI/fuzz and funded operation still precede activation.


## Later explicit owner choice: overdue collateral default

The owner answered the separate recovery question:

> Use remaining collateral to compensate lenders, less the capped keeper fee; borrower gets zero after grace, even if the latest price appears healthy.

This explicitly selects the proposed deadline-plus-24-hour backstop. Lock
`default_at = deadline + 86,400 seconds` and the bounded keeper settlement terms
when new quote-native debt opens. At/after that time, an unpaid position can be
closed through a distinctly named overdue collateral default: keeper receives the
locked capped reward, lenders receive the remaining collateral, borrower receives
zero, and the unresolved token-principal claim/loss is accounted exactly once.
This may forfeit healthy borrower equity; it is a fixed-term debt consequence,
not proof that the token is worthless, insolvent or untradeable. Price/history/API
failures cannot advance the deadline. Full-token repayment may win the atomic
race before default; the second terminal attempt must fail without another payout.

This resolves the financial choice previously marked pending. Implement and test
the exact path, immutable terms, lender-share/receivable/epoch accounting, distinct
event/API/UI disclosure and independent funded recovery CLI before admission.
It does not itself qualify implementation, supply actual keys/settings, deploy
code or waive final CI/review/fuzz and funded commissioning.
