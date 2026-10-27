# Maintained-provider capability request — unsent draft

**Superseded for the current implementation:** the owner selected in-house price
reporting and a simple initial host. This external-candidate material is retained
as research; the capability inquiry was never sent. Follow the
[in-house design](INHOUSE_PRICING_AND_LIQUIDATION.md) for current work.

Proposed recipient: RedStone integrations team through its
[official contact page](https://www.redstone.finance/contact/).
RedStone documents Solana and custom integrations, but no evidence yet establishes
that it offers the report below. This draft is a capability/pricing inquiry, not a
provider selection, contract, payment, feed creation or deployment authorization.
Sending it requires the owner's explicit outreach instruction and chosen reply contact.
No private repository, code, audit packet or unpublished finding is included.

## Message

Subject: Custom signed fixed-quantity Jupiter observations for a Solana protocol

Hello RedStone integrations team,

We are evaluating a maintained oracle provider for a Solana lending/shorting protocol.
We need to assess whether you can produce and support signed Jupiter-derived,
quantity-specific observations, rather than only a token/USD spot price.

For each immutable market/mint/token-program configuration and registered raw token
debt quantity q, the proposed report would bind a versioned search policy and identify
candidate WSOL input required to receive at least q net tokens. A qualifying candidate
must pass bounded instruction/account checks and simulation; expected output alone
is insufficient. We need signed source-observation context/freshness and coherent
observation epochs, not just a fresh signature over potentially old data.

The financial lower/central reference policy is separate: a feasible candidate route
is not proof of globally cheapest acquisition or a fair-price lower bound. Please
clarify which reference methodology and evidence your service can support, and which
would need a custom task/integration. API errors, incomplete searches and unknown
availability must remain distinct from an explicitly attested no-route predicate.

Could you provide:

1. A concrete proposed report schema and proof of which fields/search policies are
   authenticated, with a Solana verifier/client example and maintained deployment/source identities.
2. Your supported mint/route/token-extension scope, raw-unit precision, amount limits,
   source timestamps/context, simulation guarantees and failure/status semantics.
3. Signing/quorum/operator independence, key/upgrade/rotation controls, replay/domain
   binding and operating/support commitments, including an outage/recovery process.
4. A devnet or no-payment evaluation path, expected integration work, update latency,
   rate/size limits and whether reports can cover every outstanding fixed quantity.
5. Current pricing and minimum commitments: integration/setup charges, per-request/feed/
   signature costs, transaction/rent costs, subscription terms and scaling limits.
   We are seeking an indicative proposal and are not authorizing a purchase.

Please distinguish existing supported functionality from custom development or
unqualified assumptions. We can then review a concrete trust/settings/cost packet
before deciding on integration and deployment.

Thank you.

## Evaluation boundary

The public [RedStone developer page](https://www.redstone.finance/developers) and
[Solana announcement](https://www.redstone.finance/blog/redstone-rwa-oracle-brings-tokenized-assets-to-solana-ecosystem)
establish a documented Solana/custom-integration route. They do not establish this
Jupiter report capability, pricing or acceptance. No outreach has been sent.
A protocol-operated attester network is a separate possible design, with explicit
operator/key/source trust and funding obligations; it is not selected by this draft.
