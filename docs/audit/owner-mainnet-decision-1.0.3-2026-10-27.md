# Owner mainnet release decision — 2026-10-27 (Europe/Kyiv)

The owner expressly selected **mainnet before an external audit and a funded bug
bounty**, with protocol fee revenue accumulating toward those later engagements.
The owner also expressly answered the cap clarification:

> Launch with existing per-market borrowing caps and explicitly disclose uncapped deposits/market count.

This supersedes the earlier delegated no-go timing, audit/contest/bounty-first
sequence and blanket prohibition on mainnet spend before an audit. It authorizes
preparing and tagging final 1.0.3 code under the
[current mainnet plan](../release/MAINNET-1.0.3.md). It is not evidence of a signed
mainnet transaction or an operational VPS; concrete signing details and costs
remain to be reviewed before financial execution.

## Accepted scope and limits

- Existing `market_max_notional` defaults to 25 SOL, counting the sum of opening
  notionals of currently open positions **per market**. It is not a limit on total
  deposits, SOL margin, marked exposure, market count, protocol-wide TVL or loss.
  Existing fee-scaled, utilization, price-impact and wallet limits remain.
- Deposits and market creation are permissionless and uncapped by this parameter.
  Service allowlists and opens pauses are not deposit/global caps.
- The release has no completed external security audit and no funded bug bounty.
  AI-assisted review/fix review remains accurately attributed, not an external
  firm's certification. Existing residual economic risks remain acknowledged.
- The newer whole-repository review remains incomplete, including unvalidated
  hypotheses and source/review gaps. This decision accepts launching before
  completion rather than declaring those hypotheses false or fixed. Historical
  provider-stopped tasks remain stopped; no rerun or reassignment is authorized
  through this policy update.
- Future protocol fee receipts are uncertain. Earmarking treasury protocol fees
  for security changes no lender entitlement or on-chain fee split; maintain
  operator/exit reserves and a separate funding ledger. No reward or audit date
  is promised before actual funding and engagement.

Regression/CI, qualified runtime/fuzzer/fixture evidence, reproducible byte
identity, current venue compatibility, real governance and funded monitored
operation are still technical requirements. A source tag is distinct from
permission to unpause markets. The owner need not repeat the generic decision to
launch before audit/bounty; missing public operator inputs and the eventual exact
transaction packet are specific outstanding execution details.

The [October 21 economic decisions](owner-decisions-1.0.3-2026-10-21.md) retain their
mechanism-specific rationale. Dated devnet approvals and receipts remain devnet-only.
