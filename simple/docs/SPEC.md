# Jupiter-only pooled prototype

Status: new implementation on `codex/jupiter-only-pooled`, separate from the never-deployed legacy program. The owner selected the fixed upfront term fee on 2026-10-28. This document defines this implementation; legacy design chapters and old qualification records do not describe it.

## Product

One permissionlessly registered market per base mint, one active token-pool generation, SOL collateral, one Jupiter Router V2 execution path. Lenders receive pool shares and claim actual paid SOL income. Supported base tokens are initialized plain SPL Token or plain Token-2022 mints with no freeze authority; extension-bearing mints and WSOL base markets are excluded. This is lending with credit and liquidity risk, not native staking or a guaranteed yield.

A valid public registration creates the market, pool and token vault atomically, subject to the creator's signed setup rent and creation-fee limits. There is no per-mint governance approval. Shorting additionally requires fresh, exact-quantity trusted price history and an executable admitted route. A listing, token name or quote does not guarantee borrowing eligibility. Deposits and market count are uncapped; borrowing is limited by immutable per-market policy, position and wallet notional caps and utilization. There is no protocol-wide exposure cap.

## One ledger

Token NAV is tracked cash plus outstanding token principal. Deposits mint shares rounded down; withdrawals burn shares against NAV, pay only available cash, and cannot remove the final shares while loans remain. Default removes unpaid token principal and distributes actual recovered SOL separately. Depositors share credit losses in their pool generation. No accrued fees, rent receivables, vesting, liquidation-share epoch conversions or token-to-SOL price mark enters NAV.

All paid opening lender fees and default SOL recoveries accrue to holders through a checked 256-bit SOL-per-share index. Claims pay whole lamports. A receipt retains sub-lamport carry while it still owns shares; its signed full exit clears that carry. The final withdrawing holder receives remaining tracked SOL dust when no loan remains. No old empty-pool fraction passes to a new cohort. This is a prospective new-model rounding term for release review; it does not waive or resolve an archived legacy finding.

Direct unsolicited token transfers mint no shares and are excluded from tracked NAV. Transfers needed by an instruction must produce their own exact new balance delta. Measured Jupiter buyback surplus is deliberately credited to current pool holders. Unsolicited transfers remain in the old vault on a wiped generation rollover; this version offers no allocation or rescue promise for those gifts. Rent and unsolicited SOL outside tracked reserves are separate from economic payouts.

## Fixed-term borrowing

For whole-token debt `q`, let `S` be actual SOL sale proceeds, `A_live` the exact-q ask upper bound, and `A_hist` the completed-history maximum ask. All values are integer raw units.

- Opening notional `N = max(S, A_live, A_hist)`.
- Required collateral `V = ceil(N * CR_open / 10000)`; the borrower supplies margin `M = V - S`.
- Upfront fee `F = ceil(N * term_fee_bps / 10000)` is paid in SOL outside collateral. The configured protocol share rounds down; lenders receive the remainder immediately.
- The borrower signs minimum sale proceeds and maximum margin, fee, account rent and temporary scratch rent. A failed condition rolls back the sale too.
- The fixed term starts at the actual opening clock. Early repayment or buyback does not refund any fee. Adding margin never extends the deadline.

Loans bind their immutable policy. Governance can publish a new policy for future opens; old loans retain their original terms. A global monotonic market nonce prevents reuse of a closed loan address. Whole-token direct repayment and borrower Jupiter buyback are available before the default boundary, including the grace period. They need no price feed. A successful buyback must add at least the whole debt to the actual pool vault; surplus tokens accrue to its lenders.

## Liquidation, expiry and default

Before maturity, both the live bid lower bound and the completed-history maximum bid lower bound must imply `V * 10000 < bid * CR_liq`. A public arm records actual clock, reference sequence and reporter epoch. A later bad reference must have a newer sequence and source timestamp/slot after both confirmation delays. A healthy arm observation clears the arm; adding margin clears it; reporter rotation invalidates it. This proves two separated authenticated observations, not every intermediate private price.

A forced Jupiter buyback also checks the actual fill. Its debt-equivalent cost is `floor(actual_SOL_spent * q / actual_tokens_credited)`. If this implies a healthy position, the pre-maturity liquidation rolls back. Overbuying tokens cannot inflate this denominator to manufacture an unhealthy fill. The full actual spend still leaves collateral, and actual token surplus goes to lenders.

At maturity a keeper may buy back the whole debt without the health test. Forced buyback spending is bounded by the trusted acquisition reference plus policy premium, and by collateral after reserving the configured keeper cap. The keeper receives `min(fixed + floor(actual_spend * keeper_bps / 10000), keeper_cap, remaining_collateral)`; the borrower receives the remaining economic collateral. No extra liquidation penalty or additional protocol charge exists.

At `deadline + 24 hours`, default is the sole economic resolution: borrower buyback, direct repayment, forced buyback and margin addition stop. Any keeper may remove unpaid token principal, receive the capped reward (using collateral as the fee basis), and send all remaining economic collateral to lenders. The borrower receives zero economic collateral even if the latest price looks healthy, as explicitly selected by the owner. Loan/exposure rent and untracked position donations return separately to the borrower. Default requires no mint, route or price feed. Actual runtime timestamps enforce the boundary; a quote cannot move it.

A wiped generation can roll only after all token assets and loans are gone. Old receipts keep their old generation SOL claims; fresh deposits use a new pool and token vault.

## Trusted history and route limits

The owner trusts the in-house reporter and its off-chain history. There is no external oracle-provider requirement. An ordinary Jupiter HTTP response alone has no on-chain authority: only the configured reporter may publish a typed reference bound to market, immutable policy, exact q, reporter epoch, advancing source slot and sequence, and actual dual-clock freshness. Missing history, stale or failed quotes are Unknown, never infinite prices. Each finite reference summarizes exactly the configured number of completed contiguous epochs and commits an evidence digest. These compact assertions do not independently prove private observations.

The stdlib Python service stores bounded exact-q history in SQLite, prioritizes active loans and searches actual ExactIn buy quotes for a SOL budget whose guaranteed output covers q. It never assumes the ignored ExactOut flag works in V2, interpolates an executable quote, or sends transactions. The Node client constructs protocol instructions and verifies on-chain lookup tables; publication and keeper submission require the commissioning integration described in OPERATIONS.md. Browser trading requests warm-up and fresh routes from the optional HTTP bridge and simulates the complete protocol transaction. Advanced raw V2 response input remains available for local testing; a hosted funded operator/bridge deployment is not yet commissioned.

Only the loan-local scratch PDA signs Jupiter CPI. Pool/loan/treasury/user financial accounts never enter its execution surface. The reviewed canonical Jupiter loader instance is checked on every trade. Its stored full digest is governance's deployment review attestation, not a per-trade rehash. A Jupiter upgrade requires renewed review before routed trades work. Direct repayment and deterministic default remain separate. The program checks measured input/output balances, closes scratch accounts, returns their separately advanced rent, and does not count prefunded scratch SOL as sale proceeds. Router instruction, route size and account limits constrain broad DEX compatibility; the current genuine-fork test is a CPMM route through Jupiter, not evidence for every DEX or token.

## Release identity

This replacement has new account layouts, instructions, program identity, clients and economics. Development package version is 0.1.0; a new major/minor release line (proposed 1.1.0) should identify it, rather than presenting it as a qualified 1.0.3/1.0.4 patch. The owner says no legacy deployment exists, so no old-state migration is needed. Do not reuse the legacy 24-hour duration or its audit statuses. Fresh final-source runtime qualification, review and 24-hour invariant evidence remain required before a stable tag/deployment proposal.

The owner still intends an initial launch before an external audit and funded bug bounty, using protocol fees to accumulate those budgets. This implementation is not an external audit, is not deployed, and has not been declared ready for unrestricted mainnet use. No tag, deployment, live financial transaction or service purchase is part of its local verification.
