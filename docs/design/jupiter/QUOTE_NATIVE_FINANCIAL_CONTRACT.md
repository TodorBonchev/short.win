# Quote-native financial contract V1 — proposal for concrete review

Status: **proposed financial contract, not enabled or release-qualified**. This
specifies the smallest complete first-release model I recommend implementing and
independently reviewing after the owner's in-house/atomic-liquidation decision.
It selects no actual reporter keys or production settings. Market admission stays
off until the owner reviews the concrete settings, trust and default consequences.
No external-firm review, transaction, deployment or old fuzz credit is claimed.

Read with [the owner decision](../../audit/owner-jupiter-redesign-decision-2026-10-27.md),
[the execution choice](INHOUSE_PRICING_AND_LIQUIDATION.md),
[the producer boundary](QUOTE_PRODUCER_CONTRACT.md), and
[the earlier mathematical draft](QUOTE_RISK_DRAFT.md). This proposal deliberately
replaces reserve-native assumptions; it does not implement them with fictional X/Y.

## Recommendation and scope

Use a distinct `QuoteNativeV1` risk/rent/default contract for admitted Jupiter
markets, with one conservative `QuotePilot` financial profile initially. Keep
whole-position atomic token repayment, existing pooled lender-share mechanics,
actual-spend waterfalls, mint safety, inventory/utilization/wallet limits, and the
25-SOL sum of booked opening notionals per market. Deposits and permissionless
market creation remain uncapped. A reporter outage may stop opens and price-based
forced actions; it must not remove borrower repayment or a fixed-term lender
recovery right.

For this first model, implement normal token buyback, a conservative finite-price
S1 gap settlement, and a separately named **overdue collateral default**. Do not
implement quote-only S2/S3 cash settlement or pretend an API error proves no route.
The overdue backstop is a material proposed economic change requiring concrete
owner review: it can forfeit healthy collateral, as illustrated below. If that
consequence is not accepted, quote-only market admission must wait for a different
specified and qualified recovery contract.

This limits initial operating coverage, not the number of markets users may create.
Broad Jupiter routing does not make every mint automatically safe or supported.

## Identity, units and immutable terms

A market policy binds chain/genesis, program/configuration, market, base mint/token
program/decimals, ordinary SPL WSOL/9 decimals, model version/epoch, publisher
permissions, source/search/reference algorithm and all financial/time/resource
settings. Raw token quantities and integer SOL lamports are distinct types.
Reports bind those identities, exact q, policy/configuration epochs, sequence,
source observation window and evidence commitments. Runtime Signer/Clock and
program-owned/PDA-checked accounts authorize updates; JSON fields do not.

A position locks its backend and policy hash, q, booked N_open, fee/rent/rate/CR/
penalty/keeper terms, deadline and default grace. Existing positions never acquire
new economic terms through a policy change. Publisher rotation is a separate
operating action: it must preserve locked terms and deliberately handle history
warmup; a new key is not a new independent oracle.

## Reference methodology: an accepted two-sided value band

For each registered exact q and a bounded coherent chain view, collect:

- `L_bid(q)`: net WSOL proceeds of a validated sell of exactly q tokens.
- `U_ask(q)`: the least observed cost among the bounded, validated candidates
  obtaining at least q net tokens. Search remains nonmonotone and incomplete.
- `C(q) = floor((L_bid(q) + U_ask(q))/2)`, using checked wide arithmetic.

Require positive ordered values, `L_bid <= U_ask`, qualified route/account/token
semantics, explicit source window/skew, and the configured band/spread limits.
Crossed, inconsistent, stale, unsupported or incomplete observations are Unknown.
Source selection and bounded search policy are committed and publicly described.
Retain exact inputs, responses, simulations/state identities and measured deltas.
Do not equate a recent HTTP fetch or blockhash timestamp with fresh pool state.

These are **operator-accepted bid/ask reference values**. A sell bid is not proof
that every possible acquisition route costs at least that amount, and a feasible
ask is not proof of global best price or future liquidity. This two-sided policy
is preferable to inventing a lower edge by haircutting one ask; it still introduces
operator, source-selection, simulation/RPC and market-manipulation trust. Multiple
keys/processes on one VPS do not remove that trust.

The current API-only search/HTTP probe cannot emit a financial Finite report merely
by ordering three numbers. A separate reviewed reference-policy evaluator must
establish the above meaning and qualified observation context. Until that exists,
Unknown is the financial outcome. No numerical haircut creates the missing proof.

### History and different uses of the edges

Keep per-q completed epoch statistics. Current-epoch reports may provide live
marks but never advance historical persistence. Require H consecutive, timely,
completed finite epochs before use. Unknown, excessive publication/source gaps,
policy/backend transitions and invalid context reset the relevant warmup/arms;
repeated cranks and retroactive reports cannot manufacture completed epochs.

Use different references for different purposes:

- Opening conservative mark: `A_open(q) = max(U_live(q), max of H completed U_max)`.
- CR eligibility: current `L_live(q)` and the lower minima of the newest n_liq
  consecutive completed epochs, all checked at the actual time of use.
- Execution acquisition reference:
  `B_ref(q) = min(U_live(q), lower_median(completed U_max over H epochs))`.
- Display/reference midpoint: median of completed central marks, clearly labeled
  as a policy estimate. It is not a cash-settlement fair-value oracle in V1.

`lower_median` means sorted element index `floor((count-1)/2)`; it never
rounds an even-size financial reference upward. `B_ref` is a historical verified
acquisition-cost reference, not the bid lower
edge and not a guarantee that a route still exists at that cost. Keeping it
separate avoids making ordinary bid/ask spread itself block every buyback under
a small delta. The median and current minimum limit transient or stale expensive
references. A real upward repricing can temporarily make the spend bound unfillable;
retry/alert and gap/default terms address that risk, rather than silently raising it.

All report accounts used for one admission decision must share the model/policy,
accepted observation epoch and bounded source skew. Exact-q eligibility never uses
an interpolated or ceiling-node lower edge.

## Admission, opening margin and explicit raw quantity caps

Each admitted market has governed raw `Q_position_max` and `Q_market_max`, a finite
set of supported position lots, and a retained report for a capacity quantity at
least `Q_market_max`. Those are explicit policy limits selected from measured
execution capacity, not an inferred burned-LP depth or a guarantee of liquidity.
Do not inherit the old 3-phi-X / 7.5-phi-X claim.

For an open of q, require:

1. Exact-q live/completed history, a safe mint, qualified routes and executable
   resource limits; no market/global/coverage pause or relevant upgrade hold.
2. `q <= tokens_free`, `q <= Q_position_max`, and
   `borrowed_q + q <= Q_market_max` with checked additions.
3. The existing token-inventory utilization ceiling and
   `wallet_q + q <= floor(Q_market_max * wallet_cap_bps/10000)`.
4. Fresh qualified capacity evidence covering the whole post-open borrowed amount.
   A larger capacity node supplies upper-only coverage by buying at least that
   larger quantity; any extra tokens follow the explicit donation policy. It
   supplies no exact-q liquidation lower reference and asserts no quote monotonicity.
5. Defined all-in premium/spread gates for both q and capacity, against the small
   reference lot q_ref. For example, require
   `U(q)*q_ref*10000 <= q*L_base(q_ref)*(10000+premium_pos_bps)` and the corresponding
   capacity inequality with `premium_market_bps`. `L_base` is the minimum live/
   completed bid reference. Use wide integer products. This is an all-in liquidity
   premium including trading fees, **not** legacy fee-free sigma.

Execute the bounded sale and measure `S_actual`, then define
`N_open = max(A_open(q), S_actual)` and
`M_required = max(0, ceil(CR_open_bps*N_open/10000) - S_actual)`.
The borrower pays the bounded margin/open fee; `V = M + S_actual` is held on the
position. Book N_open once and require the market sum, including this open, to
remain at most 25 SOL. Keep the reviewed minimum notional and any stricter pilot
position-notional cap.

This is **snapshot D4**, not the old post-sale reserve-derived B_live guarantee.
Remote observation lag and route interactions can change the actual post-sale
buyback cost. The accepted oracle lag, all-in buffers/capacity and margin policy
must be reviewed and tested as new assumptions. No source tag restores the old
locked-depth or post-trade-price proof.

## One conservative financial profile; no automatic A/B upgrades

Use a separate `QuotePilot` profile, initially proposed to borrow the financial
numbers of legacy D (the most conservative existing row). It is not a PumpCurve
venue, a claim of curve reserves, or permission to apply the curve-gamma rule.
Do not promote it to A/B from an API TVL or a quote-implied synthetic reserve.
Any later profile/tier needs explicit admission criteria and independent review.

Risk/source backend identity must be separate from the informational financial
profile so old enum values, mocks and legacy tier semantics are not reused.

## Price-independent notional rent

Use locked opening SOL notional as the quote-native rent weight. In a distinct
versioned basis, accrue utilization-time indices
`J0 += WAD*dt`, `J1 += ceil(WAD*min(U,kink)*dt/10000)`, and
`J2 += ceil(WAD*max(U-kink,0)*dt/10000)`.
For a position, use
`R_accrued = ceil(N_open * dot(locked_rate_coefficients, J_now-J_open)
                  / (10000*YEAR_SECONDS*WAD))`,
then `R_due = max(R_accrued, r_min_locked)`.
The proposed minimum is the same formula evaluated at opening utilization for the
locked minimum-rent seconds. Retain checked wide arithmetic and carried lender
rounding; continuous lender credit weights become
`sum(N_open * coefficient * (10000-rent_protocol_bps_pos))`.

Do not reinterpret a field named raw token q as lamports or mix these aggregates
with legacy q-times-price rent. This changes economics: rent follows opening
notional and utilization, not the token's subsequent price. It continues through
outages without fabricating a token price. The ordinary split and rent true-up /
minimum-rent top-up / receivable accounting still require their explicit proofs.

## Ordinary exits and S1

Borrower-authorized in-kind repayment returns all q net tokens and charges full
accrued rent within the borrower's bound; it needs no report or route. A bounded
borrower buyback likewise needs borrower consent, actual full-q delivery and
rent/payment limits, not permissionless health pricing. No hosted order or
persisted partial repayment exists.

Permissionless CR liquidation requires both the live cheap-edge CR and each of
n_liq completed lower-edge CRs to be strictly below the locked CR_liq. Compute
`V_eff = max(V-R_due,0)`. The program computes the predicates; publishers do not
submit a liquidation boolean. As an additional borrower protection, a non-expired
CR liquidation whose actual filled acquisition cost makes CR healthy must revert.
This actual-cost check does not prove global best execution.

Bound acquisition spend by the lesser of the separately computed historical
acquisition reference widened by the locked delta and the actual collateral/keeper
budget. Preserve the existing close/liquidation/expiry actual-cost waterfalls,
including their locked splits, fees and rewards. Use the versioned all-in
acquisition marks where the legacy code expects fee-capped risk marks; do not claim
the old capped-venue-fee protection survives. Expiry uses the locked time and the
reviewed expiry reference/delta; preserve the ordinary expired/CR-penalty precedence
only when the necessary finite health evidence exists.

For the initial isolated Jupiter route scope, venue-side native rent is zero;
transient scratch rent must be exactly funded and refunded. Explicitly verify
both peak funding ability and final net position debit. Network/priority fees are
an operator-payer budget, not an invented token buyback cost or hidden vault debit.

S1 is a separate finite-evidence gap path: normal liquidation/expiry eligibility
must hold, and the live and completed cheap reference edges must exceed
`V-K_s` (plus only any expressly permitted nonrefundable execution rent). Arm and
recheck over n_settle distinct completed epochs and an explicit elapsed-time/gap
policy. The position stores its last checked model/epoch and consecutive count;
the eight-entry report ring is not itself a forty-epoch proof. Advance only from
new verified completed epochs, reset on an unverified gap or margin addition, and
never count repeated calls. Any batched catch-up must verify every counted epoch
still present; otherwise restart rather than extrapolate missing predicates. A position margin addition or invalid/unknown evidence refutes/resets its
arm. At execution recheck everything; pay the locked K_s and all `V-K_s` to the
lenders, with borrower payout zero and the actual token-principal loss explicit.
Use the existing settlement share-epoch/receivable mechanics only after proving
the new notional-rent inputs and loss accounting. Do not synthesize reserves or a
constant settlement box.

S2 and oracle-only S3 are unavailable in V1. No quote failure, excessive search
budget, fresh signature on stale data or unfilled route creates either condition.
This removes a fair-value cash-settlement surface; it does **not** itself provide
lender recovery during an indefinite outage.

## Explicit overdue collateral default, separate from liquidation

Recommended backstop for concrete owner review:

- Lock the ordinary maturity deadline at open and a proposed **24-hour grace**
  (`86,400` seconds), then `default_at = deadline + grace` by checked arithmetic.
  The proposed QuotePilot maximum duration is 72 hours, so the earliest such
  backstop is 96 hours after opening. If a later reviewed profile/ABI permits a
  shorter maturity, it must retain its disclosed deadline plus the locked grace;
  this proposal adds no hidden borrower-selected-duration ABI.
- Before default_at, borrower repayment, permitted buyback, margin addition and
  ordinary qualified liquidation/expiry/S1 remain possible. Alerts must give the
  borrower a usable last opportunity. Margin addition can fund rent or enable a
  buyback; **it does not extend maturity or cure unpaid token principal**.
- At/after default_at, if q has not been repaid and the position still exists,
  anyone may execute the separate overdue-default instruction. This relies on
  the pre-agreed clock and unpaid debt, **not** a claim that the token is worthless,
  insolvent, untradeable or unavailable from every route.
- Compute the same locked settlement reward `K_s(V)` with its fixed/bps/cap terms.
  Pay `K_s` to the executing keeper and `V-K_s` to lenders; pay zero to the borrower.
  Resolve the whole q claim once using explicit default/loss accounting. Do not
  label it a token repayment, S3 or fair-value settlement. An event/API/UI must
  identify `OverdueCollateralDefault` separately.
- Full q repayment can still win the race before foreclosure lands. The first
  successful atomic terminal instruction closes the debt; the other fails without
  a second payout. A stale/false no-route report cannot move default_at earlier.
- Governance cannot retroactively shorten grace or change the payout of open debt.
  Report/payer outages do not remove this clock-based right, although funded callers
  and chain availability remain operational prerequisites. Publish the SDK/CLI
  recovery path so an independent funded caller can act through another RPC when
  the single operator host is down; a clock-based right is not an automatic job.

**Healthy overdue example:** collateral V=4 SOL, debt replaceable for 0.2 SOL,
accrued rent small, but the price service is down. Ordinary successful repayment
could preserve roughly 3.8 SOL of borrower equity. If the borrower does not return
q and overdue default lands after deadline plus 24 hours, lenders instead receive
all 4 SOL less K_s and the borrower receives zero. This is collateral forfeiture
under a fixed-term debt contract, not a conclusion that the position was unhealthy.
It must be prominent before opening and in wallet review, countdowns and alerts.

**Gap example:** V=4 SOL and replacement cost has jumped to 10 SOL. A cash default
paying less than 4 SOL cannot buy the q tokens owed. Lenders bear that shortfall.
Neither the borrowing cap nor the backstop guarantees principal value or liquidity.

This backstop is simpler and more deterministic than a new no-route fair-value
oracle or asynchronous auction. It is also harsher to an overdue healthy borrower
than the current fair-value S2/S3 path. Do not hide that tradeoff or enable it based
only on the earlier generic request to simplify. The final concrete owner review
must explicitly accept this consequence, or choose and qualify another recovery
model before new quote-native markets open.

## Backend transition and retained obligations

Keep one market identity, lender-share ledger and wallet exposure across Pump
graduation/DEX routing. Existing legacy positions keep their original pricing,
rent basis and exit/default terms, including the retained direct/reference path.
The simplest first transition is to pause new opens until all legacy debt is zero,
then activate the quote-native basis/policy for new positions. A graduation with
open Pump debt therefore may delay quote-native borrowing; it does not rewrite
that debt or force a settlement during an observation gap.

Supporting both models concurrently instead requires separate typed rent indices,
aggregates and obligation groups. Do not obtain that behavior by summing q-weighted
legacy and N_open-weighted quote rent or by reinterpreting reserved bytes. Economic
policy/grid changes must retain reports for outstanding quantities; zero-debt
adoption does not erase lender claims or excuse sweep/vesting/epoch obligations.

## Concrete pilot candidates and request budget

These are **review/qualification candidates**, not authorized live settings:

| Setting | Candidate and provenance |
|---|---|
| Financial profile | QuotePilot: existing D numbers, CR_open 40,000 bps, CR_liq 17,500 bps, penalty 1,000 bps, max duration 259,200 s; no curve identity implied |
| Open fee / rent split | Existing defaults: 25 bps open fee; 1,500 bps protocol rent share; existing 50/20/30 penalty split |
| Rent curve / minimum | Existing D coefficients 10,000 / 37,500 / 550,000 bps, kink 8,000 bps, minimum 14,400 s, now weighted by opening notional |
| Token utilization / wallet | Existing 9,000-bps utilization and 2,500-bps wallet share of explicit raw Q_market_max |
| Notional limits | Existing minimum 0.1 SOL and market sum 25 SOL; proposed pilot per-position booked notional ceiling 1 SOL |
| Lots / raw capacity | One supported position lot q_lot per initially covered market; capacity report at Q_market_max=4*q_lot; wallet raw limit then equals one lot. Calibrate raw q_lot from admission evidence; do not hardcode a token decimal/display amount |
| Premium gates | Proposed all-in position 300 bps and market 1,000 bps, numerically inherited from D impact caps but explicitly **new semantics** |
| History | H=8 completed finite epochs, 2-second epochs as a load-test target; n_liq=3 and n_settle=40 retain existing counts but imply about 6/80 seconds, not old 3/40-second timing |
| Freshness / cadence | Qualification targets: age <=4 s and 16 slots; source window <=2 s and 8 slots; update gap <=3 s and 12 slots. These are independent caps, not proof of a fixed slot duration. Missed targets disable use; do not silently widen them |
| Execution delta / reward | Existing delta_liq=200 bps, delta_exp=100 bps and locked keeper terms; prototype Jupiter venue-side native rent=0, with measured temporary-rent bounds |
| Overdue grace | Proposed 86,400 s; borrowed from an existing duration constant for review, **not** inherited old settlement semantics |

Initially provision **one covered/admitted quote market**, not a global market-
creation/TVL cap. With q_lot and 4*q_lot there are two q feeds. At most three buy
candidates plus one exact-q sell per feed is eight Build calls per refresh: a
2-second target averages 4 RPS. Reserve at least 3 of the approved 10 RPS for exits,
limit all periodic work to at most 6 RPS under one shared limiter, and leave
headroom. Quote responses, simulations, retries and source-context collection
must actually meet latency/cadence and monthly-credit budgets. The average formula
is not proof that a serial worker, 10-RPS key or monthly allowance can meet it.
The current report core requires source-window start, source-window end and the
publication's runtime clock to belong to the same epoch. Thus the proposed 2-second
epoch requires the whole relevant observation and transaction landing to finish
inside its epoch; a 4-second age cap does not permit late publication. Align cycles
and measure worst-case quote/simulation/landing latency. If that target cannot be
met reliably, keep admission off and review a different cadence/risk profile or
capacity plan; an average RPS calculation cannot waive this constraint.

Four sustained Build requests/second is about 10.368 million requests per 30 days;
a 25M-credit allowance covers that only if the average cost is at most about 2.41
credits/request before other API use. Verify current endpoint credit charges and
actual billed usage; do not treat credits as requests or promise this rate is
included in the approved monthly baseline.

For M covered markets, K registered feed quantities, r actual HTTP calls/feed and
refresh interval t, nominal request rate is M*K*r/t before retries/opens/exits.
Arbitrary unique position sizes multiply the reporting duty; the per-q library's
ability to allocate many states is not a production coverage promise. Add lots or
markets only after measured capacity and actual funded coverage; upgrade API/RPC
plans through the approved purchasing process if needed. All existing q reports
remain serviced through debt resolution when new lots are introduced.

## Implementation and final review gates

Before admission, implement and independently check: source/reference-policy
semantics; current/completed history and unknown/reset behavior; coherent aggregate
coverage; all-in premium and snapshot margin math; notional-rent carries and
receivables; whole-q/actual-spend execution; finite S1 arms; clock-based default
and its borrower disclosure; legacy zero-debt transition; operator coverage and
actual delivered alerts. Verify all relevant pool/share/exposure/epoch accounting
on every terminal path and that missing/unknown references never force a cash exit.

The owner review packet must identify the exact policy hash, reporter public keys,
source/search/simulation rules, numerical settings, raw lots/caps, expiry/grace and
full-collateral-default examples, funded operators, measured request/latency limits,
byte identities, review/CI/fuzz evidence and actual deployment/configuration packet.
Do not call the model ready or enable markets while any of these is only a draft.

## Operational cost addendum (same proposed financial policy)

The two-second/two-feed profile above remains a candidate, with no production
cadence or spending approval. The [measured publication-cost record](../../audit/probes/price-publication-cost-2026-10-27/README.md)
shows continuous 30-day base signature fees of 12.96 SOL with separate updates,
or 6.48 SOL with qualified batching. A two-Finite-report v0 packet is 1,226 bytes
without compute-budget instructions, 1,278 bytes with the normal limit/price pair,
and 1,188 bytes with a synthetic four-address ALT. No real ALT or timely landing was
qualified. These exclude priority fees, retries, rent and other transactions.

The owner-approved 245.20 USD hosting/API baseline excludes SOL fees. Actual
publication duty cycle, cost/funding, payload/batching design and observation/
landing feasibility require review before final settings selection; reducing
cadence or changing history/trust semantics is not silently authorized by this
measurement. This addendum changes no numerical financial rule in the proposal.
