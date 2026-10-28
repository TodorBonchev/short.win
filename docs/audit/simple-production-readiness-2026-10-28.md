# Pooled production settings and current-route qualification — 2026-10-28

The owner requested production settings and current-route qualification, then
explicitly required sell/full buyback coverage for small tokens on active Pump.fun
curves, PumpSwap and high-volume tokens. The owner selected a **free Jupiter key**
and preparation of role setup with public production identities unassigned.
This record covers the new `simple/` program only. No tag, deployment, funded
mainnet transaction, paid provider purchase or external-firm audit is claimed.

## Concrete production proposal

Review [the policy, service, payer and role manifests](../../simple/config/README.md).
The proposed terms are 24h fixed term plus exact 24h grace, 1% upfront SOL fee
(80% lenders / 20% protocol), 250% opening / 200% liquidation collateral ratios,
0.1–1 SOL position notional, 5 SOL per-market booked notional, 20% wallet share,
and 60% utilization **at opening**. Keeper reward is 0.0001 SOL plus 0.5% of its
specified basis, capped at 0.002 SOL. Acquisition premium and spread are each 5%.
Deposits, market count and aggregate protocol exposure remain uncapped.

The service requires three completed 60s epochs, 30s AND 100-slot freshness and
10s AND 10-slot CR confirmation. Initial Free-key commissioning admits **one exact
(market, policy, quantity)** identity. It tries account budgets 16 then 32 once,
excludes the actually incompatible Scorch family, and filters above 40 total
returned metas (the program retains 48). Ordinary RouteV2 scalar/amount/slippage/
zero-fee/step checks apply before pricing. A captured 46-meta route exceeded the
actual 1,232-byte packet despite its quote; a smaller meta count alone is still
not packet-fit proof. Real lookup tables and full transaction simulation remain
mandatory. Rate limits, authentication failures and provider outages do not cause
account-budget retries.

The actual model hash is pinned in the policy JSON and independently checked by
`verify-production-proposal.py` and the service's `check-policy`. A model change
requires fresh local history. Normal observations need two logical quotes;
account fallback and a capped growth probe can require six HTTP requests. An
execution buy search can separately need sixteen. A one-identity worst spacing
sum of 22s precedes RPC, network, simulation and scheduling latency, so actual
throughput remains a commissioning gate. More configured slots do not prove
coverage. Keep an old policy's producer available until its loans resolve, or
qualify parallel per-policy coverage before changing the price model.

## Changes needed by real route evidence

Metadata-only Token-2022 mints are now admitted: canonical initialized state,
no freeze authority, maximum 4096-byte mint allocation, only MetadataPointer (18)
and TokenMetadata (19), complete bounded TLV/UTF-8/Borsh and embedded-mint checks.
Transfer-changing and unknown extensions remain excluded. New Pump metadata is
not a token price or identity authority. The browser and HTTP bridge share the
same eligibility envelope. Mutable metadata can later change eligibility; direct
repayment and deterministic default retain their separate contracts.

Jupiter's Pump adapters require a writable **loan-local** authority and native
SOL for DEX account initialization/extension. The separate explicit integration
mode derives that System-owned, empty-data PDA; financial accounts never enter
Jupiter. Untracked prefunding is recovered separately. A signed native
`route.setupLamports` allowance is bounded at 5,000,000 lamports. The transaction
payer advances that plus two scratch-account deposits within signed
`temporaryFunding`. Actual native spending stays outside sale proceeds, WSOL
buyback cost, opening notional, collateral, lender fees and keeper reward bases.
Unused setup and scratch deposits return to the payer. Exact loan and authority
balance/owner/data postconditions remain enforced; all failures roll back.

This is a bounded native-spend allowance, not a proof that every possible route
recipient spends only refundable rent. The observed Pump charges are retained in
actual instruction logs. Loan-local auxiliary accounts can incur setup again for
each new loan or DEX. Wallet review displays the separate maximum and consumed
setup is nonrefundable. The initial keeper native setup allowance is zero; a
nonzero subsidy requires explicit funded commissioning and may exceed the keeper
reward. Neither lenders nor borrower collateral silently reimburse it.

Private healthy observations now stay off chain. Warm requests have expiring
publication intents; usable confirmed references satisfy requests without new
transactions. Only needed opening/arm/clear/force snapshots publish. A fresh
private healthy/Unknown observation vetoes this operator's use of an older
adverse snapshot; other callers remain governed by the on-chain freshness
window. Publication is confirmed and the loan/reference reread before any priced
action. Deterministic default remains independent of prices and Jupiter.

Durable pre-submission reservations bound new reference addresses to four per
persistent operator database. Publication fee reservations have a 1,000,000-
lamport UTC-day ceiling; unknown outcomes keep reservations. Dry-run projects
these bounds without consuming allowance. A reference's observed storage deposit
is 1,691,640 lamports, greater than a minimum loan's 200,000-lamport protocol fee.
This and native DEX setup need working capital; fee self-funding is not proven.
Operator pass funding treats the maximum native setup as potential permanent
spending, in addition to network fees and temporary peak requirements.

## Evidence and limits

The [current capture archive](simple-current-route-evidence-2026-10-28/README.md)
retains actual mint/curve/pool state, current loader/ELF bytes, raw quotes, all
accounts, slots, lookup tables, hashes, failed attempts and reconstruction steps.
The [independent AI review](simple-current-route-evidence-2026-10-28/independent-review.md)
is accurately attributed; it is not an official external audit report.

`simple/tests/runtime/current-routes.json` is the executable compatibility matrix.
The clean-clone materializer verifies raw bytes and reconstructs archived RPC
snapshots without network access; the actual-SBF replayer checks sell, whole-q
vault credit, actual paid fees, collateral, rent, complete post-CPI rollback,
immutable token metadata, base-token conservation and combined external native
SOL/WSOL liquidity flows. Positive packet checks use actual captured on-chain
lookup tables, with synthetic fallback reported separately and disallowed by the
positive CI matrix. Scorch negatives require the exact downstream failure, not
just any failing exit code.

Snapshot captures may span slots. Local signers, lender inventory from existing
supply, collateral and history attestations are synthetic functional inputs.
These tests prove the retained candidate execution, not mainnet transaction
landing, real three-epoch history, maximum-size liquidity or all Solana mints.
The small-curve near-1-SOL probe did not guarantee full debt inside the spread
cap; no larger short is promised from a successful smaller one.

Final local verification passed **70 Rust tests**, **17 service tests**, **six
configuration guards**, **26 client/operator/HTTP tests**, **nine web tests**, full
workspace Clippy/format and web builds. The fresh 32-case actual-program smoke
finished in **21.821183799 seconds** with zero failures, anomalies, setup failures
or invariant failures. It remains a short historical-CPMM scenario smoke, not
24-hour current Pump route coverage. Exact [local receipts](simple-production-evidence-2026-10-28/)
retain command outputs, summary/completion and hashes.

The final current-snapshot matrix passed **six positive routes and two exact
Scorch refusal regressions** on the new ELF, with a 500-bps functional spread
ceiling. All positive transactions used real captured lookup tables and fit.
The other policy terms and two-epoch history remain synthetic functional inputs;
this is not a real production three-epoch history receipt.

| Current path | Sell / whole-q buyback | Opening native setup charged | Buyback native setup charged |
| --- | --- | --- | --- |
| BONK / JUP / RAY, retained Whirlpool/CLMM candidates | Passed for all three | 0 SOL | 0 SOL |
| Active legacy SPL Pump.fun bonding curve | Passed | 0.00219936 SOL | 0 SOL |
| Graduated PumpSwap pool | Passed | 0.0018444 SOL | 0 SOL |
| Active metadata-only Token-2022 Pump.fun curve | Passed | 0.0018444 SOL | 0 SOL |

The Pump openings signed a 0.005 SOL setup allowance and refunded all unused
funds. Their same-loan buybacks signed zero setup and succeeded. Four explicit
setup-budget refusal branches and a failure **after successful DEX CPI** verified
full rollback. These specific sequences support the initial zero-setup keeper
proposal; a future alternate route may require separate qualification/funding.

Current program: **786,864 bytes**, SHA256
`8c4ad9af77afa32ae09ae48f5fdcfce868bd3759c96affac975130c63781a00d`.
Current IDL SHA256:
`9b262cb8ddfe2d55ada06c31aa11049044515c9a6b85324b6f2167b17c61cc26`.
The primary price model hash is
`8cfc3b1258a12e2cc9abf5a5ebc8ffd363e034406d2b4465448b8cd6db9c7d96`.
Fresh hosted CI and independent artifact-byte comparison are recorded after the
preparation commit is published. Earlier b875/cda1/a81d/ebb28 intermediate results stay tied to those
bytes; no old fuzz duration qualifies the changed program.

## Remaining release work

1. Select/custody production program and role keys, fill public identities and
   reviewed loader/genesis values, and freeze the actual declared program ID,
   version, policy and service model. Test identities are not substitutes.
2. Reproduce final ELF/IDL and obtain green current CI, including the retained
   current-route matrix and metadata/native-setup regression tests.
3. Freeze source, runner and current external inputs and complete a fresh >=24h
   whole-system campaign with readable summary, duration, real coverage, zero
   failures/anomalies and retained hashes. The prior 32-case smoke is not this.
4. Configure the free API/RPC locally; commission funded payer balances, exact-q
   admission, public warm/rent capacity, monitoring and delivered alerts,
   restart/recovery, backups and exact running client/reporter/keeper artifacts.
   Measure history plus action throughput and retain receipts. Code tests prove
   none of these operating conditions.
5. Present the resulting candidate and a distinct stable release line (proposed
   1.1.0) for owner review. Tag only the qualified frozen commit; deploy/initialize
   only after the separate production decision and operating prerequisites.
   Changing a declared ID or runtime input requires appropriate fresh evidence.

The owner's borrowing-capped mainnet-before-external-audit/bounty policy is
preserved. Earned protocol fees may fund later engagements; lender assets,
collateral and operating reserves are not that revenue. Legacy findings and
archived review checksums are unchanged. The primary checkout and its untracked
`tests/litesvm/tests/fork_seed.rs` remain preserved at SHA256
`96518cdd9950c24b5e27ca5bf361d40a9dc80b6dd2e7debf33ad55c8d7539ef9`.
