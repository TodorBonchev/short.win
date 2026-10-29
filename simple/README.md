# short.win — Jupiter-only pooled implementation

The independent implementation selected on 2026-10-28 lives in this workspace,
on `codex/jupiter-only-pooled`. It does not import the legacy financial program.
The active candidate is untagged **1.1.0**, with generated public program ID
`BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh`. Its selected deployment
key remains private outside Git. The old worktrees and release evidence remain
preserved as historical material.

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

The bounded monitoring queue now supports safe slot reuse: complete finalized
state must prove no debt or pending publication, both clocks must expire finite
references, and any warm intent must be resolved or expire. Re-admission uses a
new generation and cold history while retaining prior evidence. Persistent
history/operator database identities and the separate
`<history>.publisher-identity.sqlite` anchor prevent accidental file replacement
from resetting publication barriers or financial allowances. The companion
`<history>.publisher-lock.sqlite` holds an exclusive lock throughout each pass.
Stop all cooperating processes and preserve a consistent, nonoverlapping backup
of history, operator, identity and lock databases with their applicable SQLite
journals. Focused queue, binding and operator integration tests cover
these rules; they do not establish live provider capacity. Defaults are
independent of price configuration, provider/API availability, Unknown prices
and price pause. Recorded local database identity loss or corruption intentionally
fails the entire operator pass until reviewed recovery. Only a fresh first boot
with no recorded history UUID may perform default-only work before later initial
history binding.

Fresh deterministic actual-SBF qualification for the selected public ID includes
six successful captured route lifecycles and two exact expected Scorch refusals.
It covers the captured BONK/JUP/RAY and Pump curve/PumpSwap/metadata-token cases,
with actual lookup-table transaction construction. This is finite captured-route
evidence, not universal Jupiter compatibility, live submission or long-run
qualification. See the [dated final preparation record](../docs/audit/simple-final-preparation-2026-10-29.md).

The candidate has not been tagged or deployed. The 24-hour qualification gate
remains **open**: unfinished combined-campaign tooling was isolated after the
provider stopped that work, and no new 24-hour run started. The existing duration
runner exercises historical CPMM scenarios and does not establish full current
route coverage. Legacy duration records, old local-placeholder captures and
pre-identity-freeze reports remain historical. Governance and operating roles,
funding, measured capacity on the real free API account and delivered alerts
remain pending. The owner's selected policy is borrowing-capped mainnet before
an external audit and funded bug bounty, with uncapped deposits and market count;
that disclosure does not satisfy these remaining release prerequisites.
