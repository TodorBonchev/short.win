# Venue and program lead triage — 2026-10-18

This is a targeted source review with the evidence stated below, not completion of the
original three-pass audit. Unreproduced claims remain open. Related known issues identify
assumptions to compare; they do not grant acceptance of a new trigger or a larger bound.
The raw reports are preserved. Draft fixes still need the release gates in AUDIT §12.

## Findings

| Finding | Disposition and evidence |
|---|---|
| AIR2-07 | **Open design weakness.** `VenueGuard` stores 12 bytes of SHA-256. A fixed reviewed ELF has a generic second-preimage work factor of 2^96; an authority selecting both candidate ELFs has a birthday collision bound of 2^48. The latter is not covered by KI-69's fixed-hash argument. No colliding executable pair was produced. Suppressing the latch also requires a changed ProgramData length; unchanged-length slot changes always flag. A full digest requires the 224-byte guard to grow to 304 bytes, a versioned migration and coordinated keeper/client changes. The source comment now states both bounds; no layout-only patch or acceptance was substituted for a migration. |
| AIR2-10 | **Open; fee mechanism identified, harmful transaction unverified.** `pump_amm::read_state` derives the creator-fee schedule from the current pool's default-creator flag. A creator initialization can therefore change risk quotes. The proposed patch changes the execution schedule as well as risk pricing, and is not safe to apply without separating those views and running a pinned-ELF healthy-position regression. Existing sale/burn protections must remain. |
| AIR2-11 | **Open; distinct risk and execution predicates confirmed in source.** `arming::inputs` uses the capped risk quoter for S1/S2 while ordinary buyback execution uses the live quote. A fee-tier boundary can separate them; a persistent reachable exit-budget interval has not been demonstrated here. Reverting the AIR-10 cap would restore the earlier sale-based issue. Retain the finding pending a regression covering both protections. |
| AIR2-12 | **Confirmed, unresolved.** `ai2_venue_review_it` executes a real PumpSwap cashback claim into an independently initialized trader-owned WSOL account with no trader signature. It transfers 545,713 lamports in the fixture. The canonical sweep leaves that balance behind; passing the alternate account to the sweep is refused. Recovery needs validated alternate accounts plus discovery, and sweep-completeness accounting must not let a diverted claim falsely clear the deposit gate. Historical already-claimed rewards make a new cumulative-counter baseline a migration question. Merely removing the ATA key check is not a complete fix. |

## Pass-1 leads

Each row records the actual review result. “Open” deliberately preserves a lead that still
needs a pinned-venue transaction, quantitative bound or a policy decision.

| Lead | Disposition / next evidence |
|---|---|
| L07 | **Open, bounded availability trigger.** `boost_blocks` reads the public WSOL balance, so donations can keep it above dust. The gate ends at `graduated_at + boost_window_s` (900 s by default); it cannot be prolonged by more donations after that deadline. Existing `pamm_it::refusals::the_boost_gate` pins the threshold and end. A permissionless-donation transaction and acceptance of that trigger remain outstanding. |
| L09 | **Open economic assumption.** A creator can receive its own fee, so the nominal fee is not automatically its net manipulation cost. Compare the fee-scaled capacity derivation against KI-15/KI-58 with creator rebates before claiming the existing bound applies. |
| L14 | **Conditional check, no current bypass established.** `pump_amm::check_trade` always checks fixed keys and only omits layout-dependent checks if the referenced venue state does not parse. That state is nonexecutable, so no trade leg runs. A different path reaching a CPI must be shown to promote this lead. |
| L17 | **Open future-layout behavior.** A pump-owned curve with an unknown discriminator becomes `NotPump` in `detect::classify_pump_with`. This needs a venue-authority layout change, not arbitrary caller data at the canonical PDA. It should fail closed in a future adapter migration; current pinned behavior is not a demonstrated bypass. |
| L18 | **Open detection assumption.** The same function returns `NotPump` before checking the canonical pool when the curve is absent/foreign. Valid canonical pools without a recognized curve need a pinned creation/transition example before changing precedence. |
| L19 | **Confirmed event inconsistency; draft correction.** Close/liquidate/expire hardcode vest destinations and zero wipe bits. The shared sweep routing and event fixes are recorded in the services triage; the dust-flush immediate-credit route also needs its own event destination. |
| L20 | **Future status hardening, no current alternate live status.** `open_short` creates `POSITION_OPEN`; exits close the account. Group with L36/L37. A new status writer or account reuse is required for the asserted current-path problem. |
| L21 | **Open re-detection policy.** Curve re-detection rewrites/initializes observation state without PumpSwap's warm-up. Restart identity now invalidates older settlement arms, but this does not itself adjudicate curve entry warm-up. |
| L23 | **Dependency-scope policy remains open.** Raydium listing validates pump precedence but gates the Raydium latch, not every pump dependency. No current malformed-state adoption transaction demonstrated. Compare with the detection assumptions in L17/L18. |
| L25 | **Permissionless first listing is intentional; adversarial pool-choice bound open.** Selection rules subsequently constrain replacement. First-caller venue/age control alone is not proof of a loss; reproduce a canonical eligible but harmful first listing. |
| L26 | **Known formula distinction, not a new fixed-supply implementation bug.** Curve synthetic pricing uses live mint supply; fee gates retain the separately documented 10^15 reference (KI-77). Keep any assertion about a different path open until its exact quoter is identified. |
| L27 | **Historical versus current synthetic state must be distinguished.** Pump v3 complete-pool pricing reads the live post-synthetic reserves; the fixed 206.9e12 seed still appears in the pre-complete hypothetical pool path. No new bound beyond PUMP-2026-10-13-1 established. |
| L28 | **No current underpayment path established.** Deposit transfers the requested amount with the pinned token program's `transfer_checked`, and the mint filter disallows transfer-fee/hook extensions. The absolute vault postcondition does not independently prove a delta, but donations do not reduce what that CPI debits. A supported token semantic that returns success with a smaller receipt is required. |
| L31 | **Open comparison of distinct quote policies.** PumpSwap risk cap at the EMA and the curve's live-supply synthetic fee are separate paths. KI-72/KI-77 disclose residuals; no broader quantitative bound demonstrated here. |
| L32 | **Open fee-only read hardening.** `fee_view` is not a substitute for the state consistency/executable check. No complete path found in this pass that trades on inconsistent pool keys solely because the fee read succeeds. |
| L34 | **Expected exit scope.** Graduation does not consult the open-only latch; a later open checks the PumpSwap mask. No latch bypass through graduation established. Revisit only with a successful blocked open. |
| L35 | **Construction invariant, future hardening.** The lending vault is bound by its market PDA and initialized by `create_market` with the mint and authority. An arbitrary token account cannot replace it. Group with pass-2 implied-owner checks; no current writer changes its authority. |
| L42 | **Open compute/resource bound.** Fee-tier copies are present. A maximal valid pinned fee schedule that exceeds the supported heap/CU budget has not been exercised; copying alone is not proof of an unavailable instruction. |
| L44 | **Within documented carried-term behavior.** A position keeps its locked execution-rent tolerance after switching to Raydium (KI-64), reducing its available buyback budget. No larger amount than that stored tolerance was established. |
| L45 | **Open opening-capacity bound.** Burning supply can move the live synthetic fee and capacity terms. This is a different use from an exit risk quote; do not close it solely under KI-72/KI-77. Needs a supply-burn, open, repayment comparison. |
| L46 | **Open curve warm-up policy.** The curve does not use the post-PumpSwap warm-up gate; freshness alone does not prove resistance to a lagging EMA after real price movement. Related to L21 and pass-2 lagging-EMA margin. |
| L47 | **Open extras hardening; no current transfer path.** Trader token accounts are absent from the explicit deny list. `parse_extras` still requires a governance-selected key/template, non-signer, non-program owner, exact writability and venue CPI postconditions. Those checks are not a proof for arbitrary future venue instructions. |
| L48 | **Open refund attribution review.** The sell refund is bounded by the trader rent floor after normalization. No additional account/lamport source yielding a harmful over-credit was reproduced. Preserve the lead rather than equating a missing local delta assertion with theft. |
| L49 | **Open pre/post supply consistency.** The post-open view retains entry supply. A pinned sell CPI that changes mint supply, or an authorized extra that does, is needed to make it stale within the instruction. No such current path demonstrated. |
| L51 | **Open margin/fee comparison.** D4 margin uses the live opening schedule, while exit risk has an EMA cap. Retain until a harmful combination is shown; applying the exit cap indiscriminately to execution or margin is not a fix. |
| L53 | **API policy.** Open bounds sale proceeds, margin and fee, not the tier enum itself. A changed tier can lock different terms. Group with L40/L52 and pass-2 missing locked-terms bound; no automatic new slippage API selected. |
| L54 | **Venue-specific policy.** Raydium's real-SOL eligibility floor is distinct from curve progression and PumpSwap's canonical locked-depth rule. No harmful eligible pool below the intended floor reproduced. |
| L55 | **Open dependency-mask policy.** A live curve open checks the curve mask, while later synthetic/graduated execution depends on PumpSwap. The new pass-2 buy-halt lead is related but is not a duplicate of a short.win latch bit. |
| L56 | **Same unresolved extras question as L47.** Trader base ATA is not explicitly denied; governance selection and current CPI invariants remain relevant. |
| L57 | **Confirmed strict composition behavior.** `parse_extras` requires exact writable equality, so transaction-wide privilege promotion can refuse a read-only extra. This is an availability/composability constraint, not an unauthorized write; policy remains open. |
| L58 | **Venue dependency availability.** Curve pricing uses PumpSwap config even before graduation. An unreadable config can stop quoting; venue upgrades are trusted dependencies (KI-44), but the small-q effect needs explicit interface review before accepting a new bound. |
| L59 | **Open post-complete invariant hardening.** Zero initial virtual quote reserve is one discriminator for post-complete pricing. Other field consistency and pinned pump-produced shapes must be tested before tightening it. |
| L63 | **Retained-age policy.** Pool adoption preserves original market graduation age on a Raydium-to-Raydium switch. The source confirms the behavior; whether a thinner old pool grants an excessive new-pool tier remains open. |
| L64 | **Future-owner hardening gap.** `pump_cashback_claimable` treats every non-pump owner as empty, whereas an absent System-owned account and an unreadable foreign-owned account should be distinguished. The canonical PDA prevents an arbitrary caller installing such an owner today. No patch or current exploit claimed. |
| L65 | **Future-authority hardening gap.** Adoption verifies Raydium vault authority; the cached fast parser subsequently checks keys/mints but not authority. A venue-authorized vault mutation is needed. Current caller substitution is blocked by cached keys. |
| L66 | **Known transition residual.** The curve's final EMA and the post-synthetic pool anchor differ as recorded in PUMP-2026-10-13-1; no expanded bound demonstrated. |
| L69 | **Open fallback reachability/bound.** Undefined migrating fair-value inputs can fall back to the full amount. Keep the distinction between a valid modeled rho and an unreadable/no-quote state; a concrete fixture beyond the existing migrating residual is still needed. |
| L71 | **No external token-program substitution established.** `mint_guard` checks the mint owner against the market's stored token program even though the exit passes that stored key. The input is an exit-path safety report, not a caller-selected CPI program. |
| L72 | **Open, grouped with AIR2-11.** A live fee move and a risk-fee cap can affect opposite settlement box corners. Do not remove AIR-10's protection based on one side of that comparison. |
| L73 | **Documented low-water chain.** The market's lowest locked fee cap starts the common S3 timer, while the position still checks its own current no-route predicate. This is KI-74/AIR-19, not an assertion that each position independently waited the whole interval. |
| L76 | **Future upstream-interface dependency.** A future cashback instruction requiring the trader signature could block a permissionless bundle; the pinned instruction does not require it (AIR2-12 exercises that fact). No current signature deadlock. |
| L77 | **Open lifecycle check.** Post-leg sync passes absent canonical pool because its instruction carries the curve trade accounts. Need a real same-transaction pool creation/transition missed by this check before changing the account ABI. |
| L78 | **Open upgrade/layout availability.** An unreadable migrating curve prevents normal sync; no new current pinned-layout path was reproduced. Preserve under the dependency review, not as an accepted generic venue risk. |
| L80 | **Open transition age semantics.** Migration to `None` does not necessarily set `graduated_at`; subsequent Raydium adoption has its own age assignment. A position/S3 sequence affected by that intermediate value is still required. |
| L81 | **Open executable-classification hardening.** Raydium token readers expose `frozen`, but the adapter's executable predicate does not use it. A frozen vault makes the token CPI fail. A pinned venue/admin path and its S3 implications remain to be demonstrated. |
| L82 | **Confirmed unsupported template placeholder.** `venues::parse` resolves `coin_creator` only for PumpSwap, not a curve. A governance extras template using it on a curve refuses instead of resolving it. This is a template capability/documentation gap; no fund movement or default substitution occurs. |
| L83 | **Reader hardening implemented.** Token-2022 reserves exactly 355 bytes for a Multisig. The reader now rejects that length even if signer-key bytes mimic `AccountType::Account`. The focused regression failed before the change; canonical key constraints still limit current program reachability. |
| L84 | **Open pinned-ELF status behavior.** AMM v4 status 5 is treated as unavailable. A real timed transition/swap at that status, rather than just a status-name interpretation, is needed to adjudicate the lead. |
| L85 | **Current alias refusal already enforced.** Anchor rejects duplicate mutable account use for a self-destination. An explicit check may help future refactors, but the reported current bypass is not established. |

## Additional implemented program hardening

- `raydium::lp_supply` now requires the exact 82-byte SPL mint layout (L38). Its unit test
  accepts a valid mint and refuses a 165-byte account with the same mint-like prefix. Cached
  venue-owned LP keys already prevent arbitrary replacement in current adapters.
- `cover_deficit` uses `market_now` for the lender touch and event, as deposit/withdraw do.
  This addresses the ten repeated pass-2 raw-clock blocks; the math calendar fix separately
  handles a calendar that has already advanced. No present Agave clock regression is claimed.
- A restart stores `restart_ema_count = count + 1` in eight reserved Market bytes. A settlement
  arm at or before the previous count must rearm even when every warm-up evaluation was
  skipped. Initialization preserves the cumulative count, and the first post-upgrade prologue
  conservatively invalidates legacy arms once. I18 now checks the restart marker on actual
  pool switches, graduations and price starts. The account remains 3,104 bytes.
- Deposit holds use -1 for initialized/no hold, zero for legacy/uninitialized, and positive
  timestamps for held vests. Zero protects an already-live pre-upgrade vest until actual
  release. The halted-S3 regression resets only the new reserved field to legacy zero in an
  otherwise instruction-reached state and checks refusal, release and initialization.

## Deliberately unresolved pass-2 program questions

The grouped pass-2 register distinguishes all source blocks. In particular, the pre-roll
PumpSwap fee cap, curve-open PumpSwap buy-halt mask, lagging-EMA opening margin and missing
locked-terms slippage bounds remain open. Implied owner/version/zero-tag/writable checks are
future hardening claims unless a current writer or transaction establishes reachability.
No report patch was applied solely because a raw block labeled itself a finding.
