# In-house pricing and liquidation option — 2026-10-27

The owner asked whether Jupiter limit orders and ordinary Jupiter quotes could
replace a separate oracle provider, with the keeper, indexer and program operated
in-house. This is a feasibility proposal. It does not select production reporter
keys, financial settings or a changed liquidation contract. No inquiry was sent.

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
