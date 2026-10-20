# Decisions required for draft 1.0.3

**Historical options, superseded by the [2026-10-21 owner selections](owner-decisions-1.0.3-2026-10-21.md).**
The owner delegated the recommended choices; the program has never been deployed.
The later record also includes the now-executed L10 trace and the accurately labelled
AI-review route. The original options and evidence below are preserved.

Prepared on 2026-10-20 from the merge at `eae77d9abec811c2a6ebb4187b95feded3014190` and its retained audit evidence. Keeper and frontend follow-ups are still being integrated. This packet presents concrete options for the owner and reviewer; it records no new acceptance, waiver, signature or release approval.

The owner confirmed on 2026-10-19 that no affected AI3-FR01 state is deployed. Recovery of that state is therefore not a prerequisite for this deployment. The implemented deposit quarantine remains necessary if an affected legacy pool is encountered later; ordinary sweeps do not reopen it. See [the correction and deployment record](ai-review-3/ai3-fr01-fix-1.0.3.md).

## 1. VenueGuard digest length

AIR2-07 and AI3-L12 concern the 12-byte SHA-256 fingerprint retained by [VenueGuard](../../programs/shortwin/src/state/venue_guard.rs). A fixed target has a generic second-preimage work factor of roughly 2^96. An actor choosing both candidate ELFs has a generic chosen-pair collision work factor of roughly 2^48. Slot, length and executable constraints also apply; no colliding executable pair is demonstrated. These work factors describe attacks on the digest, rather than a measured exploit cost.

The existing KI-69 bootstrap decision does not settle the chosen-pair question. The [unresolved treatment](ai-review-3/unresolved-1.0.3.md) preserves that distinction.

The owner can choose either of these paths:

- Retain the truncated digest for 1.0.3 with a written acknowledgement of the chosen-pair threat, the venue upgrade-authority trust assumption, and the limits of the latch guarantee.
- Require a versioned full-digest migration before release, including authenticated old accounts, rent/CU measurements, and compatible program, keeper, governance and client readers.

A layout-only change would not complete the second option.

## 2. Risk fees and executable repayment

AIR2-10/11 and AI3-L13/L19-L21 must be considered together. [PumpSwap state reading](../../programs/shortwin/src/venues/pump_amm.rs) uses the current creator and fee schedule. [Curve inputs](../../programs/shortwin/src/venues/pump_curve.rs) also depend on current fee state. Risk assessment uses a capped view, while the live buyback must pay the executable fee. The [venue triage](ai-review-2/triage-venues-2026-10-18.md) identifies these distinct predicates; the [handoff](handoff-1.0.3-2026-10-20.md) records the creator-change evidence and its limits.

A fee or creator change can alter these views. The available evidence does not establish a worst harmful healthy-position decision, or the amount and duration of a persistent interval in which every permitted exit is unaffordable. Existing KI-72/KI-77 decisions cover their specified cases, rather than every asserted crossing.

The owner can choose either of these paths:

- Retain the present risk/execution distinction for 1.0.3 and acknowledge its precisely documented fee-change and availability limits after the supported crossings below have been measured.
- Require a joint redesign of risk history, fee selection and executable budgets before release, preserving protection against sale-based manipulation and accounting for the actual CPI charge.

Before either can be described as bounded, execute a supported creator change and relevant multi-tier supply/fee crossing against the pinned venue ELF. Record both settlement corners, the honest counterfactual, live repayment budget, S1/S2/S3 alternatives, duration and borrower/lender amounts. Deleting creator fees or removing the risk cap alone does not supply that evidence.

## 3. Cashback outside the canonical account

AIR2-12 is reproduced by `air2_12_permissionless_cashback_can_be_stranded_outside_the_canonical_ata` in [the venue regression](../../tests/litesvm/tests/ai2_venue_review_it.rs). It sends 545,713 lamports into a trader-owned noncanonical WSOL account which the canonical sweep cannot recover. That amount is a fixture sample, not a maximum. Canonical claim completion can consequently differ from historical reward completeness.

The owner can choose either of these paths:

- Require discovery, authentication and recovery of alternate accounts before release, with historical entitlement and claim-counter migration specified.
- Retain canonical-only recovery for 1.0.3 and explicitly acknowledge stranded value and the risk of later admission before every historical reward is accounted for. No universal amount or recovery-time bound is established.

A recovery design needs split claims, alternate-account closure, account substitution, repeated claims, correct old-owner attribution and new-lender admission tests. Relaxing the canonical ATA validation alone is insufficient. AI3-L22's external claim availability remains a separate dependency even if alternate-account recovery is implemented.

## 4. Keeper payment after rent exhaustion

AIR2-13 is pinned by `air2_13_rent_exhaustion_pays_no_keeper_even_with_a_floor` in [the accounting regression](../../crates/shortwin-math/tests/ai2_accounting.rs). With a 20 SOL vault, 19 SOL buyback and 1 SOL rent, the keeper receives zero despite a nominal 0.21 SOL floor. [The waterfall](../../crates/shortwin-math/src/waterfall.rs) pays that floor from remaining equity. The accounting balances; independent keeper service is not guaranteed by it.

The owner can choose either of these paths:

- Keep rent-first allocation and acknowledge that some otherwise valid exits pay no keeper reward. Record the operational service/subsidy assumption and monitoring response.
- Require a reserve, subsidy or priority change before release, explicitly assigning the cost between borrowers, lenders and the protocol.

The second path needs payout and affordability regressions across age, tier and exit route, plus instruction budgets. The first requires no claim that an external keeper will execute at a loss. KI-03's loss disclosure is not an independent acceptance of this service assumption.

## 5. Historical pot entitlement

AIR2-P2-02 is reproduced by `pass2_pot_claims_can_use_receivables_and_the_proposed_current_share_subtraction_does_not_fix_it` in [the accounting regression](../../crates/shortwin-math/tests/ai2_accounting.rs). Two old lenders accrue 200 SOL; a later depositor joins; 100 SOL is realised while 100 SOL remains receivable. A 50 SOL pot permits the old claimants to receive 75/25 SOL. The tested withdrawal and subsequent unpaid-rent reversal leave pending balances of +21, +43 and -64 SOL.

The first claimant's extra pot draw is bounded by available unmatched value at the time. No small absolute bound or complete net attacker-profit calculation is established. The current-share subtraction proposed in the audit still permits the full 50 SOL draw.

The owner can choose either of these paths:

- Keep the existing first-claim pot policy and explicitly acknowledge that cash can pay historical receivables early, with order-dependent allocation and later deficits.
- Require historical receivable/realised entitlement tracking or a different cash-realisation policy before release.

A redesign needs claim-order permutations, deposits/exits, covers, absent lenders, rebases/wipes, pot-only chains, conservation and migration tests. KI-53's existing shortfall/debt decision does not independently decide the cash-priority question.

## 6. Carry allocation to the final holder

AI3-F06 clears pool carries at a final withdrawal. Its remaining ownership rule is reproduced in [math](../../crates/shortwin-math/tests/pool_carry.rs) by `late_depositor_can_capture_preexisting_carry_unresolved_residual`, and in [signed transactions](../../tests/litesvm/tests/lender_it/adversarial.rs) by `late_depositor_captures_preexisting_carry_unresolved_residual`.

A holds 2e18 raw tokens while a 1,001,000-lamport sweep fully vests. A receives 1,000,000 lamports; B, which deposits one raw token after that vest and exits last, receives the preceding 1,000 lamports left in carry. This preserves the chosen final-holder rule, but does not reconstruct historical ownership.

Under the documented ratio-cap and u64 mint-supply assumptions in [the handoff](handoff-1.0.3-2026-10-20.md), one index's whole-lamport carry is at most about 18,446,744 lamports (0.018447 SOL); the initial share ratio gives about 18,446 lamports for a fresh pool. Those are conditional per-carry bounds, not a lifetime aggregate cap. Entitlement and realised indices describe the same vault value and must not be added as two independent cash losses. The 1,000-lamport sample alone does not establish net profit after fees.

The owner can choose either of these paths:

- Retain final-holder allocation and explicitly acknowledge that a late holder can receive pre-entry unallocated carry, within the stated assumptions and with no aggregate bound across repeated cycles.
- Require historical carry allocation before release, including share-change accounting, already allocated claims, backing and migration analysis.

The AI3-FR01 quarantine solves admission to an already-empty legacy pool; it does not decide this ownership rule while shares remain.

## Exact comparison for AI3-L23

The original two-release trace is a duplicate of owner-accepted KI-68 within its exact bound. [KI-68](known-issues.md) records the 2026-10-08 decision: a current or parked lender floors each touch, so an intervening touch may lose at most one lamport of entitlement and one of realised allocation. Its 37 versus 0+36 example and AI3-L23 have the same cause. The forward-chain cumulative-floor path is separate.

Recommended register treatment is to identify the original L23 claim as that scoped duplicate, preserving the original report and existing owner decision. No new acceptance is needed for the identical unit behavior.

The [independent accounting review](ai-review-3/independent-accounting-1.0.3.md) also records accumulation. With two equal one-token lenders and one-lamport releases, touching one lender after every release leaves it unpaid by 500 lamports after 1,000 releases, or 10,800 after a default 21,600-second vest. The control pays both lenders equally. Both final global carries and diagnostic dust are zero. These totals follow the existing per-touch mechanism; they do not exceed its unit bound.

For fixed shares and positive index increments, n floors differ from one combined floor by at most n-1 whole lamports per index. There is no constant lifetime cap. Keep aggregate behavior, changing shares and any remainder redesign as a separate investigation. KI-68's fee explanation is not a complete proof against all batching or economic strategies: transaction fees apply to transactions, and multiple instructions can share a transaction. The new measurements do not demonstrate profit after fees. A remainder change would affect program math, historical ownership, regression vectors and the final-byte fuzz gate.

## Evidence still needed for other leads

| Item | Concrete work that can proceed | Decision or limit |
|---|---|---|
| AI3-L10, repeated donations | Use the production deposit-hold path and pinned transactions. Start a historical settlement/reward vest, repeatedly transfer/sweep, and measure actual vest end, blocked deposits, donor spend, fees and final old/new lender payouts. Cover one-lamport proposals where valid, funded trader accounts, repeated cycles and end-of-vest boundaries. Extend the existing [hold regression](../../tests/litesvm/tests/ai2_halted_s3_vest_it.rs). | No universal cost/duration bound is established. Retain the actual-vest gate during measurement. Choosing historical-reward isolation or accepting continued deposit denial is an owner liveness decision; a timestamp-only bypass can restore AIR2-04/05. |
| AI3-L16, tier resources | Establish the largest fee-tier schedule the pinned venue admin instructions can actually create. Measure that schedule on every affected short.win read/exit path, including allocations, CU and both curve and PumpSwap schedules. The [parser](../../crates/shortwin-venue-state/src/pump.rs) bounds list shape by account bytes; the adapters copy tiers to heap collections. | A u32 count and account-length bound do not establish fit in the supported heap/CU budget. Synthetic large account data alone does not establish venue reachability. Arbitrarily rejecting more than today's tiers could make future legitimate exits unavailable. Stream/bound only after compatibility and resource evidence. |
| AI3-L22, external claims | Reproduce a currently supported reward that cannot be claimed; record the upstream refusal, retry duration, canonical/alternate balances and existing lender entitlement. Test discovery/retry and admission after recovery alongside the [cashback tests](../../tests/litesvm/tests/ai2_venue_review_it.rs). | Canonical sweeping does not establish upstream claimability. No universal recovery deadline is known. Do not clear the reward gate merely because the claim path is temporarily unavailable. |
| AI2 terms/configuration leads | Extend the validated L40/L52 configuration trace in [the accounting tests](../../crates/shortwin-math/tests/ai2_accounting.rs). It locks 257,534,247 lamports minimum rent per 1 SOL notional at 89% utilisation under an extreme valid configuration, versus 13,698,631 at zero utilisation. Measure the default/deployed terms separately. | This is configuration sensitivity, not evidence that current defaults are unsafe. Borrower bounds on locked terms or relational configuration limits require a specified API/economic policy. Do not close these leads through an unrelated fee decision. |

The remaining substantive AI2/AI3 leads still need the exact dispositions in [the AI3 register](ai-review-3/triage.md) and [the AI2 register](ai-review-2/triage-pass2-2026-10-18.md). The six choices above do not grant blanket acceptance of those inventories. Already accepted KI-69/KI-70 decisions also remain within their original bounds.

## Required release evidence

The retained 24-hour run and the current program are different artifacts:

| Evidence | Program identity | What it establishes |
|---|---|---|
| [Completed all-profile fuzz](fuzz-1.0.3.md), 86,405 seconds, zero failures/anomalies | `9b91060`; release `b88aa520c24301522b45871e73622a4d498603dc03a8f34ff8bbb557d7fa0647`; test venue `994ad12c468f465937af21080ec3c48678bb1f197ab143ab44f91f98ca56132b` | Historical clean-run evidence for the candidate before AI3-FR01. |
| [AI3-FR01 CI and verified artifacts](ci-1.0.3-fr01-hashes.txt) | `e1d66caa`; release `eb9cd3e68c33de8119b11f978ab77f65d4fa41de79904c508f0fd9d519d737dd`; test venue `cf1c44424ff1c879ac5639b432a25c0dc4a81c5842ce047e26668ef920030322` | Exact-ref green CI and verifier/Anchor equality for the quarantine. It does not supply the missing final-byte 24-hour run. |

The minimal remaining path is:

1. Finalise the keeper/frontend follow-ups and every selected program or fuzzer change. Record the candidate revision and actual release, test-venue, mock and fuzzer binary hashes.
2. Obtain exact-ref full CI for the integrated commit, including the affected keeper/browser tests, mutations, dependency checks and verifier/Anchor equality. Reuse prior individual evidence only where its source/artifact identity is established.
3. Complete at least 24 hours of the all-profile runner on the frozen final binaries. Preserve source revision, commands, start/end times, elapsed duration, runner/artifact hashes, summary, log, coverage, failures/anomalies and completion status. A previously started FR01 run can qualify if its completed record proves those exact final bytes; a reported 20-hour checkpoint cannot.
4. Obtain the auditors' fix review required by [AUDIT section 12](../AUDIT.md#12-logistics-and-change-policy), covering the AIR2/AI3 changes and keeper follow-up. Internal AI review supplies evidence for that review; owner risk acknowledgement cannot substitute for it.
5. Record the owner's exact economic choices and resulting dispositions, then verify release hashes and deployment order, including indexer support before program sweep destination 3.

Any subsequent program or fuzzer byte change requires the final-byte 24-hour gate again. Keeper/frontend-only follow-ups do not themselves reset that gate when the frozen program and fuzzer identities are unchanged; they still require their appropriate tests and integrated CI. The AIR-01 through AIR-24 historical waiver does not extend to this batch.

No findings are marked fixed or acknowledged by this packet.
