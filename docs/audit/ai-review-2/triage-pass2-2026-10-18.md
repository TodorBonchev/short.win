# Pass-2 grouped triage register — 2026-10-18

The immutable [raw report](pass-2-unjudged.md) contains 330 agent blocks. This inventory maps every block to one of **74 exact source keys**: 53 full blocks (34 keys) and 277 one-line restatements (42 keys), with two keys present in both groups. Repeated reports are not independent reproductions. This is targeted triage, not the missing original judge run or pass 3.

Evidence and limits: [math/accounting](triage-math-2026-10-18.md), [venues/program](triage-venues-2026-10-18.md), [services/governance](triage-services-2026-10-18.md). Draft means implemented in the committed candidate, not released or auditor-approved. New internal tracker IDs AIR2-P2-01 and AIR2-P2-02 distinguish the stronger allowance case and the pot/receivable case from the original 13 findings.

| Exact source key | Full blocks | Restatements | Track | Disposition |
|---|---:|---:|---|---|
| `shortwin-math\|allowance\|allowance-cap-counts-locked-shares` | 2 | 7 | AIR2-P2-01 / L39 | Confirmed one-token top-up bypass beyond KI-13; draft cap uses unlocked-share value. |
| `shortwin-math\|claim-sol\|hardening-inconsistent-time-base` | 1 | 0 | L15/L16/L79 | Calendar hardening; math evidence. Program-only claims also require the clock assumptions. |
| `shortwin-math\|claim\|pot-draw-against-receivable` | 1 | 0 | AIR2-P2-02 | Confirmed unequal allocation; unresolved. The report’s proposed subtraction does not fix the reproduction. |
| `shortwin-math\|convert-steps\|lock-rounds-down-at-conversion` | 0 | 9 | AIR2-01 | Confirmed rounding mismatch; draft nonzero-lock ceiling, with zero locks preserved. |
| `shortwin-math\|deposit\|vested-principal-diluted-by-late-deposit` | 1 | 0 | AIR2-04 | Draft all-settlement hold and legacy-vest initialization. |
| `shortwin-math\|evaluate\|arm-clock-survives-restart` | 0 | 7 | AIR2-08 | Confirmed; draft persistent restart marker invalidates skipped-warm-up arms. |
| `shortwin-math\|expire\|expiry-reward-below-liquidation-floor` | 0 | 9 | L41 | Reward difference reproduced; competitive keeper-delay loss remains unproven. |
| `shortwin-math\|expire\|hardening-rate-unchecked` | 1 | 0 | math hardening | Draft rate validation; current program parameters already bound the rate. |
| `shortwin-math\|liquidate\|hardening-rate-unchecked` | 1 | 0 | math hardening | Draft rate validation; current program parameters already bound the rate. |
| `shortwin-math\|liquidate\|hardening-unchecked-rate` | 1 | 0 | math hardening | Same rate-validation group; preserve the distinct source key. |
| `shortwin-math\|liquidate\|keeper-floor-junior-to-rent` | 0 | 10 | AIR2-13 | Zero reward at rent exhaustion reproduced; liveness policy remains open. |
| `shortwin-math\|min-notional-buyback-room\|buyback-room-assumes-zero-overshoot` | 1 | 0 | min-notional | Confirmed zero-overshoot boundary; configuration/policy remains open. |
| `shortwin-math\|ratio\|hardening-zero-tag-full-wipe` | 1 | 0 | epoch tags | Future/malformed-input hardening; current writer reachability reviewed in math evidence. |
| `shortwin-math\|settle\|flush-skips-post-wipe-sweep-gate` | 0 | 10 | AIR2-05 | Confirmed; draft gate and immediate credit, coordinated program/keeper/web/indexer handling. |
| `shortwin-math\|settle\|vest-flush-per-settlement-ratio` | 1 | 0 | AIR2-04 | Confirmed split-settlement bypass; threshold removed in the draft. |
| `shortwin-math\|sol-settle\|s3-wait-from-market-chain` | 1 | 0 | L73 / KI-74 | Shared low-water timer is documented; a higher-cap position has no separate full-duration history. |
| `shortwin-math\|touch-pot\|hardening-chain-epoch-unchecked` | 1 | 0 | epoch chain | See math evidence; do not infer a current writer for malformed records. |
| `shortwin-math\|withdraw\|dust-position-blocks-last-lender-flush` | 1 | 5 | L43 | See math evidence; retain stronger dilution claim separately from intentionally deferred flush. |
| `shortwin-math\|withdraw\|stale-lock-era-b-min` | 1 | 0 | lock calendar | See math evidence; retain any distinct reachable stale-era claim. |
| `shortwin\|add-margin\|hardening-zero-valued-tag` | 1 | 0 | L20/L36/L37 | Future status-tag hardening; only POSITION_OPEN is currently written. |
| `shortwin\|claim-sol\|hardening-clock-not-clamped` | 0 | 11 | L15/L16 | Math calendar hardening; raw program clock remains a distinct reviewed assumption. |
| `shortwin\|claim-sol\|hardening-inconsistent-time-base` | 0 | 4 | L15/L16/L79 | Calendar hardening; math evidence. Program-only claims also require the clock assumptions. |
| `shortwin\|close-short\|carried-position-stale-ert` | 2 | 0 | L44 / KI-64 | Stored execution-rent tolerance survives adoption; no enlarged bound established. |
| `shortwin\|close-short\|hardening-deny-list-gap` | 1 | 0 | L47/L56 | Open future extras capability; no harmful current CPI established. |
| `shortwin\|close-short\|hardening-event-destination-hardcoded` | 0 | 6 | L19 | Draft accurate routing and post-sweep bits. |
| `shortwin\|close-short\|hardening-status-unchecked` | 0 | 10 | L20/L36 | Only current open status is written; future status hardening. |
| `shortwin\|cover-deficit\|hardening-clock-not-clamped` | 10 | 0 | clock hardening | Draft market_now clamp, plus math calendar hardening; no present clock rollback claimed. |
| `shortwin\|deposit\|hardening-balance-check-not-delta` | 0 | 12 | L28 | Pinned transfer_checked plus mint extension filter provide the current transfer guarantee. |
| `shortwin\|deposit\|hardening-version-unchecked` | 1 | 0 | account versions | Open future MarketVault version hardening; distinct from the VenueGuard constraints fixed in this batch. |
| `shortwin\|expire\|hardening-event-destination-hardcoded` | 5 | 0 | L19 | Draft shared sweep destination and wipe-bit reporting; includes immediate lender credit. |
| `shortwin\|liquidate\|hardening-asymmetric-zero-price-sigma` | 1 | 0 | zero-price sigma | Open hardening: extreme near-zero quote returns a different refusal; no eligible pinned pool reproduced. |
| `shortwin\|liquidate\|hardening-deny-list-gap` | 1 | 0 | L47/L56 | Open future extras capability; current governance key selection and CPI invariants still apply. |
| `shortwin\|liquidate\|hardening-event-destination-hardcoded` | 5 | 0 | L19 | Draft shared sweep destination and wipe-bit reporting. |
| `shortwin\|liquidate\|hardening-implied-owner` | 0 | 5 | L35 | Vault construction invariant; no current caller replacement demonstrated. |
| `shortwin\|liquidate\|hardening-pre-roll-ema-fee-cap` | 1 | 0 | fee timing | Open one-roll fee-tier discrepancy; no harmful pinned transaction established. |
| `shortwin\|liquidate\|hardening-sigma-zero-price-revert` | 1 | 0 | zero-price sigma | Same extreme-price group; no current eligible-pool path established. |
| `shortwin\|liquidate\|hardening-status-unchecked` | 0 | 7 | L36 | Future state-machine hardening; no alternate live status writer. |
| `shortwin\|liquidate\|hardening-unmapped-sigma-div-by-zero` | 1 | 0 | zero-price sigma | Same extreme-price group; preserve source key. |
| `shortwin\|liquidate\|hardening-version-unchecked` | 0 | 5 | L37 | Future position-layout compatibility; no current incompatible account writer. |
| `shortwin\|max-in-liq\|carried-position-stale-ert` | 0 | 2 | L44 / KI-64 | Stored execution-rent tolerance; no expanded bound established. |
| `shortwin\|open-short\|burn-raises-open-fee-cap` | 0 | 3 | L45 | Open capacity-specific supply-burn bound. |
| `shortwin\|open-short\|curve-open-ignores-pamm-buy-halt` | 1 | 0 | L55 dependency masks | Open policy: curve checks create-pool halt, not every future PumpSwap buy dependency. |
| `shortwin\|open-short\|hardening-asymmetric-warmup-gate` | 0 | 6 | L46 | Curve warm-up policy remains open. |
| `shortwin\|open-short\|hardening-deny-list-gap` | 0 | 7 | L47/L56 | Open extras hardening; no harmful current CPI demonstrated. |
| `shortwin\|open-short\|hardening-implied-owner` | 1 | 0 | L35 | Vault PDA construction currently guarantees mint/authority; future explicit-constraint hardening. |
| `shortwin\|open-short\|hardening-invariant-unchecked` | 0 | 5 | L48 | Refund attribution review remains open. |
| `shortwin\|open-short\|hardening-stale-mint-supply` | 0 | 4 | L49 | Needs a pinned CPI that changes supply within the open. |
| `shortwin\|open-short\|hardening-version-unchecked` | 0 | 7 | L50 | Draft VenueGuard reader version constraint; real instruction regression added. |
| `shortwin\|open-short\|lagging-ema-open-margin` | 1 | 0 | L21/L46 | Open held-price/EMA lag case; timestamp freshness alone does not adjudicate it. |
| `shortwin\|open-short\|margin-fee-view-asymmetry` | 0 | 4 | L51 | Open margin/risk/execution schedule comparison. |
| `shortwin\|open-short\|missing-locked-terms-bound` | 1 | 0 | L40/L52/L53 | Confirmed configuration sensitivity; term slippage/configuration policy remains open. |
| `shortwin\|open-short\|missing-min-rent-bound` | 0 | 7 | L40/L52 | Extreme valid configuration reproduced; terms policy unresolved. |
| `shortwin\|open-short\|missing-tier-bound` | 0 | 7 | L53 | Open locked-tier/terms API policy. |
| `shortwin\|open-short\|pumpswap-no-real-sol-floor` | 0 | 6 | L54 | Distinct venue eligibility policies; no harmful current example established. |
| `shortwin\|open-short\|stale-ema-open-margin` | 0 | 3 | AIR2-02 | Draft freshness checks plus two-step idle refresh. |
| `shortwin\|open-short\|venue-gate-mask-asymmetry` | 0 | 5 | L55 | Dependency-scope policy remains open. |
| `shortwin\|pump-cashback-claimable\|hardening-owner-fail-open` | 0 | 1 | L64 | Future owner/layout hardening; canonical PDA blocks caller substitution today. |
| `shortwin\|run\|start-sample-pins-warmup-seed` | 0 | 1 | AIR2-03 | Draft exclusion of start sample; original ELF regression reproduced. |
| `shortwin\|sol-settle\|fee-view-mismatch` | 0 | 6 | AIR2-11 | Open risk/execution budget interval; do not undo AIR-10 without both regressions. |
| `shortwin\|sol-settle\|hardening-conditional-vault-store` | 0 | 8 | L70 | Current helper mutations reviewed; future API-shape hardening, see services evidence. |
| `shortwin\|sol-settle\|hardening-unchecked-writable-account` | 1 | 0 | record writer | Future mutability hardening; needs a successful unauthorized write, not merely an omitted local assertion. |
| `shortwin\|sol-settle\|hardening-vacuous-token-program-check` | 0 | 8 | L71 | Mint owner is still checked against stored program; no caller substitution established. |
| `shortwin\|sol-settle\|risk-fee-tier-lowered-by-same-tx-buy` | 0 | 6 | L72 / AIR2-11 | Open two-corner fee comparison. |
| `shortwin\|sol-settle\|s3-wait-from-market-chain` | 0 | 4 | L73 / KI-74 | Shared low-water timer is documented; a higher-cap position has no separate full-duration history. |
| `shortwin\|sol-settle\|vested-principal-diluted-by-late-deposit` | 0 | 12 | AIR2-04 | Draft all-settlement hold including pre-upgrade vests. |
| `shortwin\|store-records\|hardening-unvalidated-writer` | 0 | 12 | L75 | Current callers load/validate records first; future helper API hardening. |
| `shortwin\|sweep-trader\|cashback-destination-not-swept` | 0 | 6 | AIR2-12 | Confirmed pinned-ELF stranded cashback; recovery/completeness migration unresolved. |
| `shortwin\|sweep-trader\|foreign-claim-dependency` | 0 | 6 | L76 | Future upstream signature/interface dependency; pinned claim is permissionless. |
| `shortwin\|sync-lender\|hardening-raw-clock-lock-calendar` | 0 | 12 | L79 | Math calendar hardening; no present Agave rollback claimed. |
| `shortwin\|sync\|s3-timer-missing-grace` | 0 | 1 | L80 | Open intermediate transition-age semantics. |
| `shortwin\|withdraw\|dust-position-blocks-last-lender-flush` | 1 | 0 | L43 | See math evidence; retain stronger dilution claim separately from intentionally deferred flush. |
| `shortwin\|withdraw\|hardening-implied-owner` | 1 | 0 | L35 | Canonical vault construction currently binds authority/mint; no caller substitution demonstrated. |
| `shortwin\|withdraw\|hardening-same-account-legs` | 0 | 11 | L85 | Current duplicate mutable account check refuses self-destination. |
| `shortwin\|withdraw\|missing-lock-bound` | 1 | 0 | withdraw terms | Open UX/API bound on the resulting lock; no new instruction parameter selected. |

Counts were checked directly against the source headings and restatement table (53 + 277 = 330). No source block was deleted or relabeled. The register preserves open questions rather than granting owner acceptance by grouping them.
