# Isolated execution prototype for the selected signed-feed architecture

No production instruction should accept a caller-supplied amount as a trusted risk
bound. The common execution layer can nevertheless be built and tested separately.

## Proposed modules

1. The implemented no_std header/fixed-meta prototype, crates/shortwin-jupiter-route:
   - pinned current Jupiter discriminator/header and fixed-account interpretation;
   - bounded lengths/account counts; canonical authority, account, mint and token
     program binding; explicit allowed fee/route features;
   - returns ValidatedHeader, certifying only its partial checks; never CPI authorization, a price or a collateral/liquidation decision.
   No HTTP SDK, price-provider dependency, state mutation or signing occurs here.

2. An internal SBF execution helper, initially reachable only from a test probe:
   execute_bounded(AuthorizedRoute, SwapAccounts, AuthorizedExecutionBounds).
   AuthorizedRoute is not implemented by the current header parser.
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

The owner selected in-house authorized Jupiter-derived reports for new-market risk,
then delegated the simple first-release liquidation model: bounded isolated
protocol-funded whole-debt buyback, preserving the actual-cost waterfall. The
[selected contract](INHOUSE_PRICING_AND_LIQUIDATION.md#first-release-liquidation-choice)
requires atomic full repayment or rollback, not a persisted partial fill. The
bounded executor can be tested before the report backend supplies financial authority.
An existing reference backend remains a possible regression/test bound source;
this does not replace the selected new-market architecture.

## Review of parent's JUPITER_REDESIGN.md

No material incorrect assertion found at the reviewed uncommitted version.
Its provider candidate and variable-override trust boundaries are properly described
as selected at the architecture level, with provider/settings unapproved and integration unimplemented. The useful additional precise contracts are:

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

## First SBF probe scope from independent implementation review

Use `tests/jupiter_exec_probe`, an explicit build script and a separate target
artifact directory. Do not add it to normal production handlers or the default
Anchor program list. Start with ordinary V2 ExactIn, two executor-created plain
PDA token accounts and a unique execution authority. Precreate all venue accounts;
allow zero venue-side native rent. Shared accounts, ExactOut, external setup/cleanup
and arbitrary intermediate accounts need separate evidence.

Construct CPI roles deliberately, then inspect them by key across duplicate metas.
A writable alias of the execution signer must be rejected, including in the opaque
tail. The current header parser intentionally does not enforce this. Never forward
market/pool/position/config/oracle/payer/borrower accounts or vault authorities to
Jupiter. Fund only bounded scratch inventory, then require actual final vault credit,
refund all remaining WSOL and apply the documented surplus policy locally.

Safe endpoint mints do not prove safe token-account TLVs. The existing base account
reader ignores Token-2022 TLVs. Initial scratch accounts must be created and checked
by the executor, with an explicit allowed account-extension policy; attacker-created
ATAs with MemoTransfer/CpiGuard or unexpected extensions are rejection cases.

Reuse normalization ideas from engine/trader.rs, but independently test program-owned
escrow-to-WSOL funding under SBF. The existing helper includes the debited position
in a local System CPI for runtime balance visibility; that precedent does not permit
forwarding the escrow to Jupiter. Check both donation-normalized token and native
lamport baselines.

Hostile SBF tests must cover incomplete input consumption, underdelivery, overspend,
substitution/duplicate privilege escalation, delegate/owner changes, early closes,
rent theft, prefunded scratch accounts, reentry attempts and atomic rollback after
postcondition failure. Label these separately from actual pinned Jupiter execution.
The latter needs retained instruction/account snapshot, ELF/IDL identity, execution
bank features, deltas and measured CU/account/data limits. No SBF probe is implemented
or passed by this document or by the header unit tests.
