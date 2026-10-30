# short.win — Jupiter-only pooled implementation

The independent implementation selected on 2026-10-28 lives in this workspace,
on `main`. It does not import the legacy financial program.
The selected software release is **v1.1.0**, with generated public program ID
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

The software release is prepared for commissioning; it is not deployed. The
[release record](../docs/release/POOLED-1.1.0.md) separates the software tag from
mainnet activation. The independently reviewed
[financial-duration campaign](../docs/audit/simple-financial-campaign-2026-10-29.md)
passed on 30 October 2026: 124,584 completed cases over 86,400.558399631 seconds,
all eight scenario families nonzero, and zero failures/anomalies/setup/invariant
failures. The runner uses historical CPMM fixtures, synthetic clocks/history/funding
and one loan per fresh VM. Current six positive/two expected-negative routes and
queue/operator checks retain their separate bounded scope. No 24-hour current
Pump/queue, real provider history, arbitrary multi-loan interleaving or universal
Jupiter qualification is claimed. Stopped combined tooling remains isolated and
unqualified; earlier duration/capture records remain historical.

Governance and operating roles, privately backed-up key custody, actual loader/
genesis settings, funding, measured capacity on the real free API/RPC accounts,
delivered alerts and mainnet commissioning remain pending. The owner's selected
policy is borrowing-capped mainnet before an external audit and funded bug bounty,
with uncapped deposits and market count; that disclosure does not satisfy the
remaining operating prerequisites. The selected financial scope superseded the
former local combined-tooling launch hold; the exact final evidence and hashes
are retained in the campaign record.
