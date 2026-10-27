# Draft: Jupiter execution and two explicit pricing models

Source under review: 3fa558a1bba4cc5884ef8d1d5a44aa1a9b642714. The owner selected
architecture B (signed Jupiter-derived feeds) and postponed mainnet. These
mathematical/ABI details remain a draft, not chosen production settings or an
independent firm's audit. Implementation prototypes do not qualify new markets.

## Direct answer

Yes, Jupiter-derived size quotes can supply margin and liquidation pricing if the
protocol explicitly trusts an authenticated quote oracle. They cannot be substituted
as an unverified request parameter or as a single token-price scalar while claiming
the current risk model is unchanged.

Current math/risk.rs3-8 explicitly accepts quote results. CR at126, margin_d4 at344,
keeper budget bounds, pprime and fair_value can consume another authenticated source.
The difficult part is producing the right Q-specific current and historical evidence.

Preserve in BOTH designs: the 25 SOL per-market booked opening-notional ceiling, current
minimum-notional/utilization/accounting checks, lender inventory requirements and the
existing SPL/Token-2022 mint safety filter. Deposits and market count remain outside that
borrowing cap. Neither design makes every Jupiter-listed mint eligible automatically.

## A. Independent on-chain reference risk, Jupiter execution

Separate two currently conflated concepts:
- risk_source: PumpCurve / reviewed reference-pool family and its canonical pool(s);
- execution_source: Direct / bounded Jupiter route.

The existing PumpCurve/PumpSwap/Raydium reference backend remains available. Add strict
readers and verified Q->buy/sell cost math for each additional reference family.
A CLMM reader needs price, active and boundary liquidity, tick arrays, fees and crossing
rules; a DLMM reader needs bins, composition and fee rules. These state families cannot
be represented faithfully by pretending their vault totals obey XY=k.

Minimum new interface is a versioned RiskAdapter: read validated reference state;
quote actual Q; determine conservative opening capacity/fees; and provide the
counterfactual or historical size-cost evidence used by margin, persistence and
settlement. The current Quoter::Pool/Curve (venues/mod.rs663) and reprice(L,P) risk
surface do not already implement CLMM/DLMM. Either derive and independently justify a
new stressed-liquidity surface per family, or use a deliberately specified cost-curve
history for those families. Do not silently plug tick/bin TVL into X/Y/locked-depth D.

State/ABI:
- Market stores risk_source and reference identity separately from execution choice.
- Quote/read accounts remain strict and keyed; new bounded tick/bin read sets need
  actual CU, account-size, transaction and manipulation qualification.
- Position locks the risk model version and its existing financial terms.
- Jupiter execution receives route bytes/accounts, bounded input and required output;
  it never supplies the authoritative quote.
- Pump graduation retains canonical PumpSwap discovery, pricing and fair-value rules,
  even when the eventual swap is executed through Jupiter.

Reference failure, freshness, pool switches and S3 remain observable on-chain. Keeping
direct execution provides a route when the off-chain router/API is unavailable. The
smallest first milestone is Jupiter execution on current supported reference markets;
CLMM/DLMM-only markets become eligible only when a risk backend is actually supported.

This design avoids a new price-attester trust assumption. It still requires new
risk math/policies where the current burned-LP depth and constant-product reprice
proofs do not transfer. It reduces per-DEX execution encoders, not all per-DEX risk work.

## B. Attested Jupiter size-cost oracle, new risk model

A concrete quote-native alternative is a market cost-curve feed. Its marks are the
protocol's agreed reference values under a published quote-search policy, NOT a
cryptographic proof of the cheapest possible swap or of future liquidity.

### What producers measure

For each fixed quantity point q_i in a bounded market grid:
- S_i: WSOL received selling exactly q_i base tokens, net of trading fees;
- B_i: WSOL input of a validated candidate route obtaining at least q_i base tokens;
- an independent small-size price band P_lo/P_hi and source/spread information;
- route-availability status at that size.

Current maintained /build is ExactIn-only. A producer can obtain an ask B_i by searching
WSOL input amounts, retaining a route whose enforced minimum output is at least q_i,
and independently simulating the actual instruction against its chain view. An
ExactOut-capable route source can be supported separately. A displayed token price
times q is not a replacement for this size-specific ask.

A feasible route proves that route's quoted cost, not that every cheaper route was
searched. Independent search/simulation rules and oracle incentives therefore matter.
Several signers all reading the same Jupiter endpoint retain its common-mode risk.

Do not infer a fee-free net cost from gross quote fields. If the new feed cannot
authenticate route fee decomposition, define its premium as ALL-IN cost over the
price band. That is a new S2/cap definition, not the old fee-free sigma with a new label.

### Minimum state and update ABI

Proposed separate accounts, avoiding a large rewrite of every existing Market field:
- QuoteOracleConfig: authorized producer set, required quorum, source/search policy
  hash, risk-model version, grid, max age/skew, update interval, circuit-breaker bands
  and timelocked rotation. These are OWNER CHOICES, not adopted defaults.
- MarketQuoteState: mint/token program/WSOL identities; model/grid/config versions;
  last sequence and context-slot range; expiry; authenticated size marks/statuses;
  per-size EMA and 8 completed observation buckets including cost/premium extremes.
- Position: risk backend/model version, opening snapshot/epoch, its Q or grid reference,
  locked financial terms and booked opening notional. Key rotation is not frozen forever
  to the key set used at open; current authorization and locked economic terms are separate.
- update_quote_curve(report, individual reports/signature proofs): only authentic,
  fresh, unique authorized signers count. Verify exact signed bytes and Ed25519
  verification instruction offsets/program identity. Publish before trading, so every
  position action need not carry all producer signatures.

Message domain includes chain/genesis, short.win program and config address, market,
mint/token program/decimals, WSOL quote units, risk/search/grid versions, sequence,
observation epoch and context slots, validity limits, all quantity/cost/status data
and source commitments. No cross-market, cross-network, stale-config or replay reuse.

One concrete quorum example for evaluation is 7 producers, at least 5 fresh individual
reports, median plus dispersion rejection, with at most 2 faulty reporters; this is
not an accepted production choice. A quorum of keys does not establish independent
data if all operators share the same API or operator. Require bounded observation
skew and published validation rules rather than an aggregator inventing one payload.

Only distinct completed epochs advance history. Reject duplicated signers/epochs,
future observations and retroactive backfill that manufactures 8 history buckets in
one transaction. Forced-decision evidence excludes the current update's own bucket,
following the current post-roll/pre-record discipline. Missing cadence resets arming
rather than counting repeated keeper calls as observations.

Grid/interpolation is an explicit tradeoff. Do not linearly interpolate arbitrary
multi-route quotes while assuming convexity. Initially either restrict Q to covered
lot sizes, or use conservative step/envelope bands for both position Q and aggregate
borrowed Q. Upper size-bucket cost is suitable for conservative margin; a lower
cost edge used to justify forced action must be an explicitly attested reference
band, not falsely claimed to follow from one feasible-route quote. Coarse bands can
make opens expensive or settlements wait; all outstanding Q must remain covered
when grids rotate. A Merkle-root feed with per-position proofs is a scaling option,
not a shortcut around these semantics.

### Actual math reuse and required replacements

Let current authenticated size-cost evidence be [C_live_lo(Q),C_live_hi(Q)], with
completed-history EMA/robust bands [C_lo(Q),C_hi(Q)]. Stale/unknown is a separate typed
status, not None=+infinity.

For an initial exact-grid prototype, each position Q must equal an authenticated
q_i; arbitrary-Q interpolation is disabled. This is a proposed admission tradeoff,
not a current protocol restriction. For aggregate opening capacity, evaluate the next
covered quantity at or above sumBorrowedQ+Q; reject if coverage ends. A future envelope
scheme needs its own proof before arbitrary quantities are admitted.

One explicit temporal contract to evaluate is:
- C_ema(Q): completed-epoch EMA of authenticated central buy marks, using a fixed,
  versioned update interval and smoothing rule;
- C_lo(Q)=min(C_ema(Q), median of prior completed bucket lower-cost extrema);
- C_hi(Q)=max(C_ema(Q), median of prior completed bucket upper-cost extrema).
Current-epoch observations never enter these historical values. Require positive,
ordered bands and sufficient distinct timely epochs. These are accepted oracle
reference bands, not guaranteed global-minimum market costs. Flash-manipulated
liquidity can affect even honestly produced quotes, so these formulas still need
adversarial economic validation; no old locked-depth proof is inherited.

A provider may expose only one signed scalar per fixed feed/job. In that case the
consumer needs a bound, versioned set of amount/direction-specific verified feed IDs,
not an imagined one-feed signed curve API. The provider spike must prove how to
assemble a coherent same-epoch vector and reject mixed/stale quantities. Fixed
immutable jobs/feeds and their source policy are part of the oracle contract.

- Opening margin can retain the D4 algebra:
  M_req=max(0,ceil(CR_open*max(C_live_hi(Q),C_ema_hi(Q))/10000)-S_actual).
  But these are oracle snapshot costs, NOT the current post-sale on-chain buyback cost.
  A stale snapshot may miss price movement during the transaction. Freshness, accepted
  oracle lag, conservative buffers and execution bounds are new economic assumptions.
- Book N_open from the authenticated conservative opening mark, lock it on the position,
  and keep sum(booked N_open)<=25 SOL. Changing from current post-open B_live to this
  snapshot mark is an explicit notional-definition change, even though the same cap remains.
- CR arithmetic is unchanged: floor(V_eff*10000/B). A forced liquidation should require
  CR(V_eff,C_lo) below CR_liq (if even a cheap plausible buyback is unsafe),
  with both fresh live evidence and the required distinct historical persistence.
  Do not force liquidation merely because an unsigned current expensive route was supplied.
- The reference/keeper-spend bound can retain the existing min formula using a robust
  past low-cost reference C_ref(Q), the locked delta, V-K_s-ert and existing rent rules.
  Actual Jupiter execution still must obtain>=Q and debit<=max_in(+explicit ert).
- Replace X/L_slow/D/fee-scaled opening-cap evidence with an explicit quote-native
  capacity policy evaluated for BOTH Q and sumBorrowedQ+Q, maximum accepted premium/
  spread and sustained depth. There is no burned-LP withdrawal-invariant D in a
  transient Jupiter quote. Keep inventory, utilization, wallet limits and 25 SOL cap;
  do not silently preserve the old 3phiX/7.5phiX economic claim.
- Tiering must use a defined quote-capacity/age policy or a deliberately conservative
  initial tier, not pretend quote depth is locked SOL. Existing SOL/USD conversion
  may still serve USD tier thresholds; it is not a token-price oracle.
- Current dual fee views also cap risk fees at locked terms while execution pays
  live fees. Gross router costs do not automatically preserve that rule. Either
  authenticate enough fee/net decomposition to retain separate execution/risk views,
  or explicitly version the replacement all-in mark and fee-rise/no-route treatment.
  Do not claim the old fee-cap protection without the necessary data.
- Current sigma(Q;P,L) is fee-free net impact. Either attest enough net/fee data to
  retain it or create a new all-in premium term with a versioned threshold. This
  choice changes S2 semantics and must be locked on positions.

### Replacing the settlement box honestly

Current settle::BoxQuotes.cost/sigma/fv_cost requires a pure Q-specific function on
hypothetical(P,L). One Jupiter price or one Q quote does not provide this surface.
Do NOT implement a fake constant cost(p,l), or synthesize arbitrary CPMM reserves.

For a quote-native backend, replace that evidence with a CostEvidence evaluator:
- Robust cost interval from prior completed per-size buckets and EMA; include explicit
  premium bands if S2 remains an all-in-premium condition.
- S1 requires insufficiency at the cheap edge of both live and historical evidence,
  plus persistence: Budget.exceeded_by(C_lo), not only the most expensive quote.
- S2 requires the low premium edge above its locked threshold across the corresponding
  evidence and persistence.
- Refutation uses evidence adverse to the proposed forced action; an amount cannot
  be finalized while the accepted uncertainty band permits materially different outcomes.
- Reuse scalar pprime(expired,V_eff,B,CR_liq,penalty,expiryFee) and fair_value(B,rate,R_due).
  For S2/S3, compute x(B)=min(V-K_s,FV(B)) over the accepted cost interval, require
  (max x-min x)*10000<=fv_dev_bps*min x, and pay the selected robust central/EMA mark.
  Evaluate endpoints AND the CR branch boundary floor(V_eff*10000/CR_liq), plus1,
  when inside the interval; pprime can change fee branch there. Do not assume two
  corners suffice without proving monotonicity for every allowed fee policy.
- This keeps the original idea of stable, persisted, amount-consistent settlement,
  but it is a new model requiring a new specification, adversarial model and tests.
  Existing(P,L) box proofs do not automatically certify it.

S3/no-route is a separate owner choice. Use per-Q status chains because a token may
route for small Q but not a large debt:
AVAILABLE / ATTESTED_NO_ROUTE / UNKNOWN(API error, timeout, staleness, disagreement).
UNKNOWN must never be interpreted as infinity, CR0 or successful no-route evidence.
ATTESTED_NO_ROUTE advances only its defined per-size S3 chain; it does not invent
a finite price or immediately create an S1/CR liquidation. S1/S2 require their defined
fresh finite financial evidence. Unavailable fair value needs an explicit separate
policy, not an accidental Option::None fallback.
If no-route cash settlement is allowed, it relies on a sustained quorum claim plus a
defined challenge/recovery window; it still does NOT prove absence of every route.
A successful actual bounded buyback for the debt can refute/reset that claim.
Alternatively require an independent reference backend for S3, or leave oracle-only
no-route cash settlement unavailable until a defined recovery process succeeds.

### Execution and failure behavior

Router quotes only select instructions. Temp authority/accounts hold exactly bounded
input; the market/pool-vault authority is never handed to arbitrary routed programs.
Verify mint/program/recipient, route arguments, bounded input, >=Q output, balances,
delegate/close authority, rent accounting and refund/sweep in one atomic instruction.
Surplus policy can preserve the current pool donation rule.

Stale/disputed oracle evidence disables new opens and forced quote-based decisions;
adding margin and safe token repayment remain possible. A voluntary in-kind Q repayment
or borrower-authorized bounded buyback can avoid an oracle-outage lock, but this is an
explicit ABI addition. Do not let anyone submit a bad route or API failure to force
cash settlement. Continuing exits during an oracle outage requires this deliberate
repayment/fallback design; signatures do not solve liveness automatically.

## Selected direction and remaining configuration choices

A: independent on-chain reference risk, slower asset expansion as reference families
are qualified; Jupiter is the execution layer.

B (SELECTED architecture): external quote oracle reference risk, broader execution-family coverage without
per-family on-chain quoting, but producer/search/censorship/lag risk becomes part of
the financial trust model and the current cap/tier/S2/settlement evidence is redesigned.

The implementation direction is a versioned risk-source abstraction and
the selected signed-feed backend, not pretending a URL quote is an on-chain price. Pump can retain
its known reference backend. The 25 SOL cap and mint filters stay in both options.
No old fuzz duration, release tag or audit acceptance is credited to either redesign.
