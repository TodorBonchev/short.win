# Unresolved findings: written treatment for draft 1.0.3

**Subsequent correction (2026-10-19):** the [AI3-FR01 guard](ai3-fr01-fix-1.0.3.md)
now refuses deposits into an already-empty pool with either carry nonzero. The reproduced
capture below describes the pre-guard candidate. The new guard preserves historical
value and claims. The owner confirmed on 2026-10-19 that no affected deployed state
exists, making current-deployment recovery/migration not applicable; affected legacy
state encountered later would still require reviewed recovery before new deposits.
Its [exact-ref full CI](https://github.com/TodorBonchev/short.win/actions/runs/39169016016)
passed with the verifier rebuild and mutations. The owner reports a separate-machine
24-hour fuzz run at 20 hours with zero anomalies; completed final-byte ≥24 h zero-failure
evidence and remaining release requirements stay open. See the
[dated status and evidence limits](ai3-fr01-fix-1.0.3.md#deployment-and-fuzz-status-2026-10-19).

Date: 2026-10-19. Reviewed code: `9b910602ed076a6d4359c496960b3ffc0e7c6af8`,
integrated with `main` at `24f514c1` in `708746eb0085016c37f4d73e2aea02031bd6efc9`.
The integration changes publishing metadata and documentation, not Rust production source.
This treatment accompanies the [independent fix review](independent-fix-review-1.0.3.md).

These are recorded dispositions, not new owner acceptances. Under
[AUDIT §12](../../AUDIT.md#12-logistics-and-change-policy) and the
[tracker rules](../findings.md), **acknowledged** requires the owner's sign-off on a
written rationale; **fixed** additionally requires regression, full gate, ≥24 h fuzz and
fix-review evidence. The request to merge and document issues supplies neither a waiver
nor acceptance of an unspecified risk. Open items below stay open, with corresponding
rows in [known-issues §8](../known-issues.md#8-open-items-from-the-103-fix-review).
The merge is integration of an unreleased candidate, not a release decision.

## Findings explicitly excluded by the packet

| Finding | Evidence and remaining exposure | Treatment and reason | Evidence needed to close |
|---|---|---|---|
| AIR2-07 / AI3-L12: truncated VenueGuard digest | `state/venue_guard.rs` retains 12 SHA-256 bytes. A fixed-target second preimage has generic work factor 2^96; choosing both ELFs has a birthday work factor 2^48. The length/slot predicate also matters: unchanged-length slot changes still flag. No colliding executable pair has been produced. | **Open design weakness.** KI-69's original fixed-hash argument and hashed bootstrap do not accept the chosen-pair case. A partial layout edit would break deployed account readers, so no unreviewed migration is included. | Versioned full-digest guard migration, authenticated old-account handling, rent/CU limits, keeper/client/governance compatibility, old/new guard tests and the fix-batch gates; alternatively explicit owner acceptance of the chosen-pair threat model. |
| AIR2-10: creator initialization changes risk fees | `pump_amm::read_state` builds the fee schedule with the current creator-default flag; `VenueState::exit_risk_cap_bps` does not freeze that flag historically. A healthy-position harmful transaction has not been independently demonstrated here. | **Open.** Preserve the live execution fee and AIR-10 sale protection. Blindly deleting creator fees would underquote the actual CPI and is not a safe correction. | Pinned-ELF creator-change transaction, before/after healthy-position/ring/settlement decisions, and separate risk versus execution policy with economic bounds and regressions. |
| AIR2-11: risk/live execution budget gap | The capped risk quoter and live buyback quoter are distinct (`venues/mod.rs`, `engine/arming.rs`, `engine/bounds.rs`). A reachable persistent interval where risk eligibility holds but every available exit fails its live budget remains unmeasured. | **Open liveness question.** Removing the cap would reopen sale-based risk manipulation. No blanket safety or availability bound is established. | Pinned-ELF fee-boundary case measuring assessment, live buyback, S1/S2/S3 alternatives, available budget and duration; a correction must retain sale protection and executable repayment. |
| AIR2-12: noncanonical cashback recovery | The historical `ai2_venue_review_it` characterization sends 545,713 lamports into a trader-owned noncanonical WSOL account; canonical sweep does not collect it. That fixture amount is not an upper bound. | **Open, reproduced accounting/recovery gap.** Retain canonical account checks; simply accepting arbitrary accounts would not solve discovery, ownership validation or deposit-gate completeness. | Validate/discover alternate accounts, recover into the correct historical entitlement, migrate already-claimed reward counters, and test split claims, account closure and admission of new lenders. |
| AIR2-13: rent-first liquidation leaves no keeper payment | `waterfall::liquidate` can exhaust available equity on rent before the keeper leg. The existing accounting regression produces zero keeper reward. This does not prove that a rational independent keeper will execute. | **Open economic-liveness decision.** KI-03's lender/protocol loss disclosure does not establish acceptance of uncompensated keeper work. No reserve or waterfall reordering is chosen implicitly. | Cost/reward analysis across age, tier and exit alternatives; owner-selected reserve, subsidy or priority policy with lender/protocol tradeoffs, executable regressions and budgets. |
| AIR2-P2-02: historical pot entitlement | `pool::claim` draws `min(need, unmatched)` against pending, without reconstructing historical receivable ownership. The prior trace pays old claimants 75/25 SOL from a 100-SOL realization while another 100 SOL remains receivable; timing changes cash allocation. This is not a complete attacker-profit calculation. | **Open accounting design.** The F06 final-share carry correction does not redesign pot ownership. KI-53's accepted shortfall/debt behavior does not by itself accept the first-claim cash race. Current-share subtraction was already disproved by the trace. | Historical entitlement or a different realization policy through deposits, exits, deficits, rebases/wipes and pot-only chains; claim-order permutations, conservation, absent-owner backing and a migration design. |

The evidence origins and exact historical tests are preserved in the
[venue triage](../ai-review-2/triage-venues-2026-10-18.md),
[math triage](../ai-review-2/triage-math-2026-10-18.md) and
[findings tracker](../findings.md#tracker). Historical observations above are not claims
that this review reran every prior proof of concept or checked current mainnet state.

## Remaining review-3 leads

This covers all 20 leads outside the implemented L01/L03/L04 corrections. Canonical
keys and the original claims remain in the [31-item triage](triage.md).

| Lead | Written disposition and retained bound | Reopen / completion requirement |
|---|---|---|
| AI3-L02, AI3-L08 | **Duplicate of KI-23 within its held-liquidity mechanism only.** Existing history manipulation is not evidence of a new profitable settlement sequence. This review grants no broader bound. | A production transaction sequence exceeding KI-23, with manipulated history, settlement outcome and net cost/profit. |
| AI3-L05 | **Deferred, open readiness hardening.** Keeper readiness caching can become stale if a previously prepared external recipient ATA closes. The specific closure-plus-failed-repair path is not reproduced. No indefinite-recovery guarantee is claimed. | Close the actual recipient ATA on a pinned venue, execute the next keeper trade, and show whether rediscovery/re-preparation recovers; test bounded retry/invalidation before changing the cache. |
| AI3-L06, AI3-L18 | **Deferred under KI-77's exact fixed/live-supply distinction.** The pinned flat curve schedule has no tier difference; that historical snapshot is not a current-mainnet assertion or a guarantee for a future multi-tier configuration. | Supported multi-tier schedule plus supply burn, exact gate/quote difference and pinned execution; do not replace live synthetic fees with an underquoting fixed-supply execution fee. |
| AI3-L07 | **Deferred decoder hardening.** A sufficiently long token-account buffer can resemble base mint fields, but the inspected callers authenticate the venue's mint key. No current account-substitution producer is established. | Any reachable path losing that key binding, or a reader contract requiring stricter mint-kind validation; then add supported Token/Token-2022 positive and substitution-negative cases. |
| AI3-L09 | **Deferred status hardening.** `open_short` writes `POSITION_OPEN`; current exits close the account. A persisted alternate live status has not been shown. | Before adding another status writer or reusable exit state, validate status on every exit and test repeated/partial exits. |
| AI3-L10 | **Open availability tradeoff.** Deposits behind the settlement/reward hold require the actual vest to finish. Repeated donations/sweeps can extend that shared vest; there is no measured universal duration or cost bound here. Relaxing the hold alone reopens historical-reward capture (AIR2-04/05). | Real repeated-donation/sweep trace measuring cost and denial duration, then historical-reward isolation or explicit owner acceptance of a quantified availability bound. |
| AI3-L11 | **Deferred vault-field hardening.** Canonical vault creation and stored PDA binding are the present boundary; no supported account importer or authority-mutating path defeating it is established. | Revisit before introducing an importer, repair instruction or authority mutation; require mint/program/authority and successful transfer-delta invariants. |
| AI3-L12 | **Open under AIR2-07**, as treated above; not closed by KI-69's hashed bootstrap. | Full-digest migration or explicit threat-model decision. |
| AI3-L13 | **Open extension beyond KI-72/AIR-10.** Live supply still affects the PumpSwap risk tier. Existing KI-72 acceptance remains limited to its documented burn/tier case and magnitude. | Supported burn/tier transition with complete liquidation or settlement decisions and measured borrower/lender effects beyond that existing case. |
| AI3-L14 | **Deferred UVA validation hardening.** Current canonical, venue-owned UVA derivation blocks the asserted arbitrary wrong-user substitution; it is not validation of all possible future layouts. | A current wrong-user creation path, or a venue interface change; test owner, discriminator, user and positive existing-UVA compatibility. |
| AI3-L15 | **Duplicate of KI-75/AIR-20 within the recorded settlement corner bound.** Transaction-controlled live liquidity is already disclosed there. No new loss exceeding that bound was shown. | Production sequence beyond the stated corner/defaults or low-lock/fee bounds, including all settlement legs and the counterfactual honest outcome. |
| AI3-L16 | **Open resource bound.** `math_tiers` copies venue-supplied tiers into heap collections. The account parser constrains data shape, but no maximal reachable supported tier count has been shown to fit or exceed the program heap/CU budget. | Establish the venue-authorized maximum, execute that schedule on the pinned ELF, and measure all affected reads; stream/bound allocation only with compatibility and resource regressions. |
| AI3-L17 | **Deferred vault-authority hardening.** Adoption authenticates the authority; no present public authority mutation of an adopted vault is established. | Revisit with a supported post-adoption authority mutation or import path; ensure stricter reads preserve exits on all supported fixtures. |
| AI3-L19 | **Open KI-77 synthetic-risk extension.** Live-supply synthetic fee selection remains; flat pinned fees do not prove safety for a supported multi-tier burn. | Supported schedule, synthetic threshold crossing and actual healthy/unhealthy/settlement decision, with measured effect. |
| AI3-L20 | **Open KI-77 gate/execution-basis extension.** Fixed-reference fee gates can differ from live-supply synthetic execution under multiple tiers. Existing disclosure is not a measured new worst-case bound. | Reproduce an admitted open or blocked exit at the mismatch, including live CPI charge and locked cap; change gate policy without underquoting execution. |
| AI3-L21 | **Open risk-history policy.** Curve risk reads current fee state while historical price/liquidity remain in the ring/box. No supported curve fee change causing the asserted wrong decision was reproduced here. | Supported fee change, historical/current quote comparison and executable liquidation/settlement trace; preserve AIR-10 protections. |
| AI3-L22 | **Open external claim-liveness dependency.** Rewards must first become claimable through the venue; canonical sweeping alone neither proves claimability nor resolves AIR2-12's alternate-account recovery. No universal recovery-time bound is established. | Current unclaimable reward reproduction, bounded recovery/discovery strategy and old-owner entitlement tests before clearing reward checks or admitting deposits. |
| AI3-L23 | **Original trace duplicate of KI-68; aggregate/design extension open.** The 37 versus 0+36 trace has the same per-touch floor and stays within the accepted bound of at most one lamport of entitlement and one of realization per intervening touch. Repetition can accumulate without a constant lifetime cap. | The [exact comparison](../release-decisions-1.0.3-2026-10-20.md#exact-comparison-for-ai3-l23) preserves that scoped duplicate without new owner acceptance. Measure aggregate strategies and changing shares separately, or design and test a remainder policy preserving conservation and historical ownership. |

## Accepted duplicates and the F06 allocation boundary

**AI3-FR01 — legacy empty-pool carry (original review).** Independent public-API
characterization reconstructs a pre-fix final withdrawal leaving `S=0`,
`carry=carry_r=1,000*WAD`, and 1,000 lamports of old lender SOL. After required sweep
completion, a new one-token cohort deposits, adds one lamport, withdraws and receives
1,001 lamports. The candidate's `deposit_in` does not migrate the old carries; its new
reset only runs inside `withdraw_in`. Fresh/post-fix cohort tests do not cover this
upgrade state. This review does not establish that any deployed market has that state.

Q1 is therefore **conditionally accepted**: the correction prevents leakage when the
final withdrawal runs the new code, but complete upgrade acceptance requires inventory
evidence proving no affected old empty state, or a separately designed migration with
historical entitlement/backing and regression evidence. Simply discarding both carries
could strand backing or erase old claims and is not an approved migration. The issue is
logged in the tracker and known-issues register; merging the candidate does not close it.

**2026-10-19 deployment follow-up:** the owner has confirmed the absence of affected
deployed state. On that confirmation, the original Q1 deployment-state condition is
satisfied for the current deployment and recovery/migration is not applicable. This
is an owner-confirmed deployment fact, not an independent chain inventory. The guard
correction and its CI/review evidence are recorded above; AI3-FR01 remains **in progress**
until the completed ≥24 h final-byte fuzz evidence and remaining gates are recorded.

AI3-F07 remains a duplicate of KI-70/AIR-04/AIR2-06: unsigned successor keys can be
installed only through approved governance, with the documented recovery path. AI3-F08
remains a duplicate of KI-69(a)/AIR-01: until hashed bootstrap an extension can latch
opens/listings/switches; exits and withdrawals are outside that guard. Those existing owner
decisions do not accept AIR2-07's chosen-pair weakness or any larger authority-transfer loss.

F06 clears whole-lamport carries on the final withdrawal using the existing final-holder
policy, and preserves already allocated zero-share claims. It does **not** reconstruct each
departed lender's fraction of still-unallocated carry. Any decision to replace that policy
needs historical allocation and migration analysis; acceptance of the cross-cohort fix is
limited to that behavior. The separate historical pot issue and per-touch rounding remain
open. The [accounting review](independent-accounting-1.0.3.md) records its direct probes.

That review also independently reproduces the allocation boundaries: a one-token late
holder can receive all 1,000 lamports of still-unallocated carry when the earlier large
holder exits first, and 1,000 one-lamport releases with an intervening touch of one of
two equal lenders pay that lender zero instead of its 500-lamport control without touches.
The other lender receives 500 and 500 remains in lender SOL. The per-touch loss stays
within KI-68's stated unit bound; the aggregate is not repaired by resetting global carries.

## Release treatment

No item becomes **acknowledged** merely by appearing in this document or the known-issues
register. Open items require their stated evidence and resolution, or explicit owner
sign-off on the exact residual and bound. No item becomes **fixed** merely because its
targeted fix review passes. The separate ≥24 h run must identify the final program and
fuzzer bytes and complete with zero failures; historical 1.0.1 fuzz and the
AIR-01–AIR-24 waiver do not cover 1.0.3. Full-gate evidence must identify the integrated
revision. No release tag or deployment is authorized by this merge.
