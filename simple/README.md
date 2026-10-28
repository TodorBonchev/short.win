# short.win — Jupiter-only pooled implementation

The independent implementation selected on 2026-10-28 lives in this workspace,
on `codex/jupiter-only-pooled`. It does not import the legacy financial program.
The old worktrees and release evidence remain preserved.

- One active pooled vault per supported token, token-denominated lender shares and separately claimable paid SOL income.
- SOL collateral, Jupiter Router V2 sale/buyback, whole-token repayment, one immutable fixed-term policy per loan.
- Fixed borrowing fee paid upfront in SOL; early close does not refund it.
- Trusted in-house off-chain exact-quantity price history; public registration and automatic eligible-quantity warm-up/publication.
- Confirmed pre-maturity liquidation, maturity buyback, then the owner's deadline-plus-24h collateral default: borrower economic payout zero after grace.
- Plain SPL and metadata-only Token-2022 eligibility (4096-byte mint limit, no freeze authority), per-market borrowing/position/wallet/utilization limits; deposits and market count are uncapped.

The new ledger has no accrued-rent receivables, vesting, direct venue adapters,
bonding-curve graduation, pool selection or legacy migration. Lender withdrawals
are limited by available liquidity; defaults can lose token principal and return
SOL compensation instead. It is lending, not native staking or guaranteed yield.

See [the specification](docs/SPEC.md), [scope](docs/SCOPE.md),
[operations and release prerequisites](docs/OPERATIONS.md), and the
[trusted history service](services/README.md). The [proposed production policy and operating bounds](config/README.md) include checked economic scenarios and unassigned, non-executable role templates. The generated client is in `client/`,
its Node-only operator supports explicit dry-run/simulation, and the minimal
wallet frontend is in `web/`. Example settings are development fixtures.

Development version 0.1.0 and the program ID are local placeholders. A distinct
release line (proposed 1.1.0) fits this replacement. It has not been tagged,
deployed or qualified by the legacy 24-hour run. Fresh final-runtime qualification,
additional Jupiter route coverage, real operator funding/configuration/alerts and
commissioning remain release prerequisites. The owner's plan to launch before an
external audit and funded bounty remains accurately disclosed.
