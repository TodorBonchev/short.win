# Smallest isolated execution prototype while pricing authority remains open

No production instruction should accept a caller-supplied amount as a trusted risk
bound. The common execution layer can nevertheless be built and tested separately.

## Proposed modules

1. A small no_std pure route parser/validator, e.g. crates/shortwin-jupiter-exec:
   - pinned current Jupiter discriminator/header and fixed-account interpretation;
   - bounded lengths/account counts; canonical authority, account, mint and token
     program binding; explicit allowed fee/route features;
   - returns a CheckedRoute, never a price or a collateral/liquidation decision.
   No HTTP SDK, price-provider dependency, state mutation or signing occurs here.

2. An internal SBF execution helper, initially reachable only from a test probe:
   execute_bounded(CheckedRoute, SwapAccounts, AuthorizedExecutionBounds).
   Ultimately bounds are constructed by a selected risk backend in an existing
   protocol handler; the constructor is not the public route payload.

Conceptual contracts (not proposed finalized ABI names):
- SellExactIn { Q, min_sol_out }: lending vault delta exactly-Q; isolated temp
  input authority funded with exactly Q; actual net output>=min_sol_out.
- BuyAtLeast { Q, max_sol_in, allowed_rent }: position funding at most the bound;
  pool vault receives>=Q; position's all-in debit<=max_sol_in+allowed_rent;
  unspent funds/refundable rent swept back and token surplus follows existing policy.
- Bind expected source/destination mint, token programs, temp authority/PDA, exact
  input/output accounts, borrower/position/market and a risk snapshot/policy digest.
- No protocol market/vault/exposure authority is a forwarded arbitrary signer.
  Reject unrelated signers and forbidden protocol-owned/critical keys; copied
  privileges must not exceed the deliberate checked CPI roles.
- Check pre/post token owner, mint, delegate, close authority, amounts and lamports;
  normalize all temp accounts, sweep leftovers and close/refund as one instruction.
  Jupiter setup/cleanup payloads are not unrestricted child instructions.

Use an isolated test SBF program/harness with explicit test-owned balances to prove
mechanics, current Jupiter bytes/IDL compatibility, bad-route rejection and resource
limits. It can receive test bounds; that does not establish any production financial
authorization. Include malicious CPI targets/routes that spend less/more, change
recipient/authority/delegate, duplicate protocol keys, steal rent, replay accounts,
or attempt re-entry. No fake oracle amount enters open_short/liquidate/sol_settle.

The first production integration can construct these bounds from the existing
independent reference backend. An externally verified quote backend can later be
wired only after its financial semantics and provider identity are selected.

## Review of parent's JUPITER_REDESIGN.md

No material incorrect assertion found at the reviewed uncommitted version.
Its provider candidate and variable-override trust boundaries are properly described
as unselected/unimplemented. The useful additional precise contracts are:

- POST-open live B(q) currently defines N_open and the D4 spot leg. A pre-open signed
  quote is a different economic mark even if the same scalar formula is reused.
- Current sigma is fee-free net impact; a gross Jupiter quote does not establish it.
- A feasible buy quote supplies one route cost, not a certified lower bound on the
  cheapest possible acquisition. Lower bands used to force actions are oracle
  economic-reference assertions.
- Current BoxQuotes needs hypothetical(P,L) size costs; the quote-native backend
  must replace that evidence explicitly, not manufacture constant-product reserves.
- Amount consistency for cost intervals must consider the pprime branch boundary,
  not assume the endpoints cover all allowed penalty/expiry-fee policies.

Independent AI policy-agent review agreed with these boundaries and the cost-edge/
pprime arithmetic reasoning. This is not a firm audit or completed qualification.
