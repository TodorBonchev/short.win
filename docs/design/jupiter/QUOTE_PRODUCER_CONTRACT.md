# Fixed-quantity quote producer contract — implementation draft

The owner selected in-house Jupiter-derived prices and later explicitly accepted
trusted off-chain price history with on-demand authenticated snapshots. An initial
trusted bid/ask policy may use strictly validated, separately collected Jupiter
sell/buy estimates, with configured buffers and spread/time limits. These are
operator-accepted reference estimates; neither edge proves a globally optimal
price or guaranteed executable output. Concrete numerical policy and its hash
still require review before activation. This contract narrows the
next producer implementation; it is not a live feed, a selected lower-reference
policy, production settings or an authenticated observation.

## Quantity contract

Use an immutable, registered raw token-debt quantity q_i. The initial producer
interface should be:

```
search_exact_q(immutable_identity, q_i, bounded_search_policy, quote_source)
    -> ApiCandidateAskUpper | SimulatedCandidateAskUpper
       | Unknown | UncoveredWithinSearchBudget
```

For a bounded set of observed candidates (input_lamports_j, minimum_output_raw_j),
select the least observed input whose minimum net output is at least q_i. Scan
that bounded set; do not assume monotonicity of routed quotes for binary search,
linear interpolation or an inferred failure bracket. A finite search can finish
without covering q_i. That is not evidence that every route is unavailable.

The maintained Jupiter Build schema distinguishes expected `outAmount` from
`otherAmountThreshold`, the minimum after slippage. An expected output above q_i
with a minimum below q_i does not qualify. The existing fixed-input outAmount
identity spike supplies neither this inversion nor B_hat(q_i). See the
[official Build schema](https://developers.jup.ag/docs/openapi-spec/swap/v2/swap.yaml).

## Report identity and observation evidence

Bind the market, chain/program/configuration, input/output mint and token program,
decimals, WSOL lamport units, exact raw q_i, search/fee/account/taker policy version,
request window and observation epoch. Retain candidate input, minimum net output,
route/response commitment and qualified source fetch/simulation context.

An API-declared route and a successfully simulated bounded execution are distinct
states. Full instruction/account validation and actual scratch/vault deltas are
required for observed feasibility; the current header prototype supplies neither.
The current Build response documents no quote-state contextSlot. A blockhash's
fetchedAt time is not a price observation timestamp. Attester/signature time does
not repair stale source data; authenticate the actual observation policy/context.

## Upper and lower references have separate meanings

A validated and successfully simulated candidate with measured net deltas supplies
an observed upper acquisition cost at that state. ApiCandidateAskUpper remains an
API-declared candidate until those checks succeed.
It is not proof of the globally cheapest route, future liquidity or a fair-price
lower edge. Failed smaller-input requests, a preceding input-ladder point or a
linear price inversion do not establish that lower edge.

Do not construct financial Finite evidence from upper-only observations. The lower
and central financial references require their own explicitly reviewed economic
policy and matching authenticated identity/quantity/epoch. Forced liquidation,
S1 and S2 remain unavailable when their required lower evidence is missing.

API errors, timeouts, stale observations, invalid configuration and incomplete
search produce Unknown/Uncovered. Invalid or case-mismatched DEX labels can produce
the same No routes found response as illiquidity in the current Build schema.
That string never establishes S3. AttestedNoRoute requires a separate size-specific,
authenticated predicate and reviewed recovery/challenge policy.

## Coverage and transport constraints

Position debt must equal a supported q_i under the current math prototype. An
aggregate capacity check may use the next covered debt node's upper mark. Keep
all outstanding quantities covered across grid changes, lock opening notional and
financial terms, and maintain distinct completed history per quantity/model epoch.
Repeated signatures or keeper calls cannot manufacture history. Pump graduation
preserves market identity and exposure; a backend transition needs explicit
continuity and warmup rules, with no automatic forced settlement during a gap.

Switchboard transport must separately prove which fields/policies it authenticates.
A fixed scalar job is not an imagined signed curve/status/route-context report.
Job identity and the consumer's reviewed immutable mapping must bind all semantic
inputs; request-scoped overrides are credential-only. See the
[provider's override trust model](https://github.com/switchboard-xyz/gitbook-on-demand/blob/6262810ebbc7feed25f04f6e98fcdeb53eaec833/custom-feeds/advanced-feed-configuration/data-feed-variable-overrides.md#trust-model).
A custom task or another explicitly reviewed report transport may be needed if
fixed jobs cannot authenticate the required search/simulation/observation policy.

## Required producer regressions before integration

Cover nonmonotone candidate responses; expected/minimum output disagreement;
incomplete ladders; raw-unit overflow; mint/quantity/policy substitution; stale
source with fresh signature; resource/fee/account mismatch; and missing-lower
refusal. No producer implementation or successful live attestation is claimed here.

## Accepted trusted-history mode

Collect two independent directions for each exact raw debt lot: a q-token sell
estimate and a bounded WSOL-input buy candidate covering q. The service may
accept their configured buffered bid/ask band as a trusted financial estimate;
authentication establishes the operator, not fair value or future execution.
Keep this explicitly separate from simulated executable evidence. An unknown
rounding rule must never be presented as a proven output minimum.

The fixed official API endpoint, validated mint/amount/taker/route semantics,
local request/RPC clock windows, search policy and raw-response digest are bound
by the configured policy. A local RPC clock proves local context, not that Jupiter
used that slot. Preserve that trust limitation in output and deployment review.
Timeouts, crossed/spread-invalid bands, stale/mismatched context and incomplete
coverage produce Unknown; they do not prove no route or authorize cash default.

Store completed history durably off chain; publish only on demand. Distinct mode,
version, immutable account/domain/q/model/configuration, monotone publication
sequence and runtime freshness checks must prevent replay/misrouting. A snapshot
may contain operator-attested completed history without any prior on-chain price
updates. Do not claim that its digest proves historical collection or omitted no
gaps. Public-history prototype measurements remain historical evidence.


The immutable snapshot wrapper also carries a per-feed `producer_commitment`,
initialized by governance. The watcher recomputes that commitment from exact q,
identity, shared financial model hash, source/history/search inputs, taker, buffers
and all request/resource settings. It must equal the actual governed account value.
The shared financial policy hash remains common across position/small/capacity
quantities; per-feed hashes cannot defeat financial coherence checks. Final market
admission must bind the reviewed feed account/commitment registry explicitly.
