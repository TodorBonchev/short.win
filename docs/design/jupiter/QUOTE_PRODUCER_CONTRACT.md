# Fixed-quantity quote producer contract — implementation draft

The owner selected signed Jupiter-derived risk feeds. This contract narrows the
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

Do not construct CostEvidence::Finite from upper-only observations. The lower
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
