# Selected in-house pricing and liquidation direction — 2026-10-27

The owner selected the in-house direction after reviewing the feasibility and
operator-price trust distinction. See the [recorded decision](../../audit/owner-jupiter-redesign-decision-2026-10-27.md).
Use a short.win price worker, our own keeper and atomic Jupiter execution. Concrete
reporter keys, financial settings and changed liquidation terms still need review
before deployment. No external inquiry was sent.

## First-release liquidation choice

The owner delegated this choice after considering partial Jupiter limit fills:
"Ok decide what you think will be best for the project and simplest". Select
full-position, atomic buyback and token repayment for ordinary liquidation. This
is the delegated engineering selection, not a claim that the owner separately
specified new financial parameters or approved production deployment.

- Keep the current actual-cost liquidation waterfall as the baseline. The program
  computes eligibility and spend bounds from accepted risk observations/history;
  a reporter cannot replace those checks with a submitted `liquidatable=true`.
  It funds an isolated executor within its absolute debit/rent limits and requires
  the pool's actual net credit
  to be at least the entire position debt `q` before releasing that debt. Surplus
  follows the existing donation policy. Jupiter may split a route internally;
  the complete transaction must still satisfy the whole-debt postcondition.
- If output is short or another postcondition fails, revert the entire transaction.
  No partial token repayment, reduced debt, partial collateral payment or pending
  hosted order survives that attempt. A failed landed transaction can still cost
  the submitting payer a network fee. The keeper refreshes evidence/routes and
  retries only within the program's current bounds and its operating budget;
  it cannot raise limits without authorization or wait indefinitely without alerts.
- Do not use Jupiter Trigger custody or asynchronous limit/stop orders for protocol
  liquidation in the first release. Avoid outstanding-order reservations,
  cancellation/refund races and mixed token/SOL debt accounting. Liquidator-funded
  delivery remains a later alternative, because a new payout rule would change
  the existing actual-cost accounting.
- A price gap can make all remaining collateral insufficient to replace `q` tokens.
  That is lender token-principal shortfall/default, even if the SOL payout is larger
  than an earlier SOL valuation. Do not describe remaining SOL as full token repayment.
  Keep cash/default settlement a separate, explicitly authorized path with its own
  eligibility, persistence, valuation, loss waterfall and accounting. The current
  S1/S2/S3 contracts are the baseline to review; they are not automatically qualified
  for a Jupiter-derived reference. A failed quote, failed swap or unfilled order
  alone must never authorize that path or establish no route.
- Before admitting any new market, qualify both the normal full-debt exit and the
  selected gap/no-route recovery policy, including stale-report behavior, deliberate
  margin-addition/token-repayment paths and operator alerts. If that recovery policy
  is not specified and tested, the market stays disabled. No new partial-repayment
  state machine or numerical settlement/risk setting is selected here.

This deliberately limits the first integration to the existing whole-position
accounting model. It reduces integration state; it does not guarantee liquidity,
continuous keeper/report availability or recovery of all lender token principal.

## Recommended shape

Operate the quantity-specific price producer ourselves. It collects Jupiter quote
candidates and validated simulation evidence, applies the reviewed reference policy
and submits an authorized price-update instruction. Standard Solana transaction
signatures plus an on-chain reporter allowlist can authenticate these updates;
a third-party oracle SDK or a custom detached-signature parser is not inherently
required. Authentication binds our reporting authority, not the truth or fairness
of the upstream quote. The operators become the oracle.

The execution keeper may remain permissionless: it scans positions and submits
candidate atomic buybacks, while the program verifies eligibility, accepted price
history, locked terms, maximum debit and actual net debt-token repayment. The
reporter key has no vault withdrawal authority. Separate report and execution roles.
The indexer reconstructs state, retains source observations/fills and alerts on
inconsistencies. It cannot replace program-enforced authoritative history with a
caller-supplied database history. Multiple keys on one VPS or under one operator
are not independent quorum.

Ordinary HTTP/API JSON can guide search and execution. The on-chain program cannot
fetch that HTTP endpoint or distinguish arbitrary submitted JSON from a genuine
Jupiter response. Financial decisions therefore require either supported verified
on-chain references or explicitly authorized in-house reports. Source/amount/mint/
policy identity, raw units, observation windows, freshness, sequence and completed
history must be enforced. The [fixed-q contract](QUOTE_PRODUCER_CONTRACT.md) still
applies: candidate acquisition upper costs do not establish the economic lower
reference needed to force liquidation or cash settlement.

If our reporter is compromised or wrong, it can cause false health decisions even
when the swap returns the owed tokens within an absolute spend cap. For example,
10 SOL collateral with a 5 SOL buyback is 200% CR; an 8 SOL reported route suggests 125%.
At an illustrative 150% threshold this reverses eligibility. These figures illustrate
the attack, not selected production settings. Actual bounded token delivery does
not repair that authorization error. Stale/unknown quotes must not become infinity
or automatic no-route settlement. Implement and test deliberate outage repayment,
margin-addition and recovery paths.

## Jupiter hosted limit/stop orders

The [current Trigger V2 overview](https://developers.jup.ag/docs/trigger) documents
Privy-managed custodial vaults, off-chain order state, USD price triggers and partial
fills. Its output quantity depends on execution. It is an asynchronous order service,
not a drop-in atomic liquidation callback for our debt/accounting contract. Moving
protocol collateral there would add custody and reconciliation obligations.

A short normally needs a buy-stop condition as price rises. An ordinary buy limit
above the current market can fill immediately; after a price gap above the limit it
may refuse to fill when protection is needed. A stop-limit also does not guarantee
liquidity or execution. short.win health depends on whole-debt buyback cost in SOL,
collateral/rent, locked terms and historical conditions. A static token/USD trigger
cannot substitute for those inputs; SOL/USD can change independently.

Prefer our own keeper orchestration and immediate atomic Jupiter swap/repayment.
An asynchronously placed order must not mark debt repaid before the full token
credit arrives. It would additionally need collateral reservation, position-version
binding, competing-close rejection, partial-fill accounting, cancellation and refund
recovery. An unfilled order proves neither insolvency nor absence of every route.

## Other execution models and their limits

Liquidator-funded acquisition followed by atomic q-token delivery and a program-
authorized collateral payment may reduce routed exposure of protocol vault authority.
It still needs correct liquidation eligibility and a reviewed payout rule; it is
not automatically equivalent to the current actual-cost waterfall.

A borrower-authorized bounded close or in-kind repayment can avoid an external
price decision. A standing collateral-for-q offer or expiry auction can use agreed
terms, but changes the economic contract and may close healthy positions if opened
unconditionally. An auction after a health trigger discovers execution offers; it
does not establish when a forced auction may start. None is silently adopted here.

The practical in-house option removes a separate oracle vendor. Jupiter/RPC and
market-liquidity dependencies, transaction/API costs and operator-price trust remain.
Concrete reporter/key controls and financial/failure settings still require review
before deployment, and new markets remain disabled until qualification.

## Simple initial deployment

Reuse keeper clients/types and build tooling, with pricing as a separately
supervised worker from the same Rust codebase. This is planned work; no new CLI
or service binary is claimed to exist. Keep a distinct scoped report key and
separate execution payer permissions. One host remains one administrative and
availability failure domain, regardless of process/key count.

The [operations plan](../../deployment/MAINNET_OPERATIONS.md#in-house-price-worker-on-the-initial-host)
uses the existing 4-dedicated-vCPU/ 16 GiB starting recommendation, remote Jupiter
routing and remote RPC simulations. Bounded request queues, shared caching and
exit-priority budgets avoid adding a message broker, separate microservice cluster
or full validator merely to collect and publish prices. Host capacity and oracle
financial/reference correctness are separate qualification questions.
