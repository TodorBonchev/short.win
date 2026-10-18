# Keeper, tooling and governance lead triage — 2026-10-18

This is a source review and targeted regression pass over the assigned pass-1 leads. Lead
numbers below are the one-based order of the bullets under `Leads` in `report.md`; they are
not the numbered findings. The reviewed tree includes the concurrent AI-review-2 fixes.
This does not claim a new independent audit or validation against a live RPC cluster.

## Changes

- `shortwin-build-alt extend/check --market` now derives the market's current addresses.
  `--table` stays the destination, and `--protocol-table` identifies the active protocol table
  whose keys are excluded. The `market` command retains its old `--table` alias. Ambiguous
  aliases, identical destination/protocol tables, and ignored market/freeze flags are refused.
  The shared selector is used by market creation, extension and checking.
- Oracle-feed staging and application now fetch the proposed account and check its receiver
  owner, decodable `PriceUpdateV2` layout, non-executable status and matching feed ID. The
  checks run again after the governance delays. They do not certify that a feed denotes
  SOL/USD or promise that its next update passes the crank's freshness/confidence rules.
- All four `VenueGuard` readers check `VENUE_GUARD_VERSION`, matching its writers. This changes
  no account layout and protects a future upgrade from interpreting an unsupported version.
- The pre-existing `shortwin-v1-probe` fix reports a failed send as unconfirmed, rather than
  saying leaders accepted v1. Source inspection confirms that only `out.err.is_none()` after
  `Chain::send` makes a sent probe successful; simulation-only mode makes no leader claim.
- Services documentation now explains market-table extension, the keeper restart needed to
  load extended ALT addresses, and the client's two-observation refresh for an idle market.

## Assigned leads

| Lead | Disposition and evidence |
|---|---|
| L01 | **Confirmed; fixed.** `keeper/src/bin/build_alt.rs` previously ignored `--market` in `extend` and `check`. Both now call `alt::selected_entries`, as `market` does. `market_alt_refresh_adds_published_extras_without_adding_protocol_keys` builds both tables, publishes a new extras set through the real instruction, detects the missing keys, extends only the market table and checks idempotence. The CLI regression pins the separate destination/protocol arguments and rejects silently ignored combinations. |
| L02 | **Confirmed; fixed in the working tree.** `keeper/src/bin/v1_probe.rs::run` now distinguishes `(send, accepted)` when printing the final explanation. `RpcChain::await_landed` returns `Dropped` on timeout and preserves confirmed status errors, so a failed/dropped send cannot produce the successful wording. This review did not send a live probe. |
| L03 | **Current behavior; policy question, no automatic clearing added.** `engine/pool_select.rs::adopt` resets `no_route_since`, `pool_fail_since` and their observation clocks, but preserves the emergency market route flag. `set_route_broken` is the authority-controlled writer. A healthy replacement pool therefore does not override an operator's market-wide restriction. Clearing it automatically would redefine that authority's scope. A stale restriction can delay recovery; no permissionless flag-setting or loss path is shown by this lead. |
| L04 | **Governance-input risk, standard tool constrained.** `state/config.rs::check_fee_recipient` accepts System-owned non-executable accounts, including System-owned short.win PDAs. It cannot establish control of an arbitrary recipient. Its comment's “every short.win account” is too broad for System PDAs. Standard ops derives Governance vault 1 (`ops/src/config.rs::fee_recipient`) and verifies its owner, rent and configured identity (`ops/src/verify.rs`); arbitrary program clients can still propose an unsuitable recipient. No general off-curve ban is appropriate because the intended Squads vault is itself a PDA. |
| L05 | **Partly hardened; semantic choice remains governance's responsibility.** `ConfigChange::OracleFeed::check_shape` only requires a nonzero account; on-chain initialization/application does not certify SOL/USD semantics. The new operations preflight rejects owner/layout/feed-ID mismatches before stage/apply. Another valid feed with a matching ID still passes, and initialization still relies on its reviewed layout values. Hardcoding a feed or removing feed rotation requires a protocol policy decision. |
| L06 | **Operational failure confirmed; mitigated in stage/apply.** A malformed/missing/wrong-owner account could previously survive both governance delays and replace the configured feed; `apply_config` clears the old cache. The new preflight runs before proposal staging and again before application. `t_ops_4_oracle_feed_is_checked_before_staging_and_again_before_apply` checks no transaction is sent for bad input and no pending proposal is consumed when the account changes before apply. Other clients remain governed by the existing on-chain checks. |
| L08 | **Conditional liveness lead remains open.** `engine/pool_select.rs::candidate_tier` reads candidate `state.r` and the candidate price used by the fit. `FitAt::Execution` removes the live-token-reserve cap, not these tier inputs. With different tier impact caps, a reserve/price movement can change eligibility. Default tier impact caps are equal; the lead does not demonstrate loss under those defaults. Existing unit test `the_fit_uses_the_tier_the_market_would_have_on_the_candidate` deliberately exercises different tier caps. The instruction's “terms no transaction can move” prose is therefore broader than its guarantee. |
| L10 | **Duplicate mechanism of L08; remains open for nondefault caps.** `tests/litesvm/tests/pool_select_it/carried_fit.rs::a_withdrawal_wrapped_around_the_execution_does_not_cancel_the_window` pins the default-cap withdrawal case. It does not prove immunity with every valid governance tier table. Choosing snapshot tiers versus live post-adoption eligibility needs an explicit policy decision and a focused nondefault-cap regression. |
| L11 | **Future-upgrade obligation.** `GlobalConfig::check_change` runs `validate_param_set` on `param_set_after`, including authority-only changes. All currently accepted states satisfy the same compiled bounds; the lead needs a future upgrade that tightens them without migrating the old config. Such an upgrade must migrate/repair parameters before relying on authority rotation. No current invalid stored config reached through instructions was demonstrated. |
| L12 | **Same future-upgrade obligation, tracked separately.** Timelock/enable/oracle changes also validate the unchanged parameter set. This can matter after new compiled bounds reject an old configuration; it is not caused by a current unprivileged call. See `state/config.rs::{param_set_after,check_change}` and its validation tests. |
| L13 | **Duplicate of L04; operational constraint documented.** A trader PDA can redirect protocol fees into later trader sweeps, and the event-authority PDA can strand them, if governance chooses it. Standard `stage-authorities` supplies the derived Squads fee vault, not an arbitrary destination. On-chain arbitrary-authority proposals retain this reviewed-input requirement. |
| L22 | **Existing oracle policy, not a new freshness bypass.** `pyth::check` bounds a price step while the cached value is at most `oracle_stale_s` old; it does not advance that cache on refusal. `oracle_max_age_s` applies to the incoming update. `oracle_crank::{a_large_step_against_a_fresh_cache_is_refused,the_step_is_not_bounded_by_a_stale_cache}` pins the boundary; the latter refuses the jump at 600 s and accepts it at 601 s under the default. Keeper oracle scenarios pin refusal/aging alerts and fallback to Tier C. A low step limit/long stale interval is a governance availability tradeoff, not silently changed here. |
| L24 | **Hardening implemented.** `create_market` now checks the guard's version before interpreting its latch bits. `ai2_guard_version_it.rs` exercises a guard with an unsupported version, representing a future upgrade-owned state. Today's writers only create the current version. |
| L29 | **Specified flag scope; no code change.** Pool selection's candidate eligibility/latch check does not require the venue's open-enable/route flag to be clear. Those flags gate new opens and contribute to S3 no-route evidence. Adopting a venue with its flag set therefore does not permit an otherwise forbidden open. Preventing that adoption, or keeping an old S3 chain across adoption, would change the selection policy. |
| L33 | **Future-loader support gap retained.** `flag_venue_upgraded`/`revalidate_venue` require the compile-time loader-v3 ProgramData PDA and owner; the keeper uses the same `program_data::deploy_slot` reader. A migration to loader v4 needs coordinated constant/parser/latch support. This review did not establish that any supported deployment has migrated and makes no live-mainnet claim. Existing latch account tests cover wrong owner/address/header, not a supported loader-v4 migration. |
| L36 | **Future-state hardening obligation.** `add_margin` checks `POSITION_OPEN`; close/liquidate/expire consume the position without that extra predicate. Current writers only create Open, and successful exits close the account. `ExitInProgress` is explicitly reserved for v1.1. An upgrade introducing that state must update every exit; no current instruction creates it. |
| L37 | **Future-layout obligation.** `ShortPosition` has reserved bytes rather than an active layout-version field. Current decoders share its pinned layout and position terms are initialized by `open_short`. A future layout/term encoding requires a migration and an explicit compatibility discriminator; the lead has no current alternative writer or layout. |
| L50 | **Duplicate of L24; hardening implemented for opens.** `open_short` now applies the same `VENUE_GUARD_VERSION` constraint as guard writers. The shared bad-version regression covers open as well as listing and pool selection. |
| L60 | **Duplicate of L29; policy retained.** `propose_pool` and `execute_pool_switch` check the candidate's upgrade latch, while open-enable and route-broken controls remain open/S3 controls. `pool_select::adopt` resets route chains, so repeated failed adoptions are a liveness topic, but the lead does not supply a successful selection attack violating the existing eligibility/depth/window rules. |
| L61 | **Duplicate version-check gap; hardened in both pool-selection readers.** `propose_pool` and `execute_pool_switch` now reject unsupported `VenueGuard.version`. No account size or instruction ABI changed. The regression expects `ConfigVersion` before interpreting candidate data. |
| L62 | **Confirmed, explicitly pinned scope distinction.** `create_market` on Raydium requires the all-Ray guard mask clear; Auto/list-only plus proposal checks the candidate bit. Existing `pool_select_it/refusals.rs::a_list_only_adoption_waits_for_the_venue_review` explicitly accepts a CPMM adoption while only AMM v4 is flagged, and refuses it while CPMM is flagged. Both-Ray open gating remains in force (`latch_raydium_it::both_raydium_bits_must_clear`). This is a listing-policy inconsistency to decide, not an opening-latch bypass. |
| L67 | **Known size/availability residual, retained.** `trimmed_elf_hash_of` returns `None` over `ELF_HASH_MAX_LEN` (2 MiB), so a changed slot fails safe as an upgrade and hash-assisted revalidation of an older slot cannot succeed. Exact-slot revalidation still works. Repeated permissionless extensions of an oversized ELF can race that exact slot; this is the already documented KI-69 size residual. Increasing the bound requires measured SBF compute evidence and is outside this host-only change. |
| L68 | **Authority trust boundary, not independent hash verification.** At an exact expected deploy slot, `revalidate_venue` trusts the authorized caller's hash and stores its prefix. After a slot move, it requires the full current trimmed hash to match. The ops command fetches and hashes the actual account and requires its deploy slot to equal the reviewed slot before staging. An arbitrary client can still give the authorized caller a bad exact-slot hash; proposal review must check those bytes. |
| L70 | **No current mutation omitted; future API obligation.** The no-settlement branch of `sol_settle` compares `lender_sol`, `unmatched`, and `protocol_fees` before writing the vault. Current prologue/accrual changes on that branch affect those tracked values. If a future prologue mutates another persisted vault field independently, expand the comparison or always store it. No present instruction trace with a lost vault mutation is supplied. |
| L75 | **Internal API hardening lead, no current unchecked caller.** `engine/lender.rs::load_records` validates owner, PDA, market, epoch and writable records; `store_records` writes changed conversion remainders back to the paired account slice. Current call sites use the same validated slice and record ordering. A future caller must preserve this contract; a validated-record wrapper would make it structural, but no current transaction substitutes accounts between these calls. |

## Keeper core review

The automatic report omitted `keeper.rs`. This pass read the actual duty loop and its supporting
code, not just module comments:

| Area | Evidence and assessment |
|---|---|
| State reads and projection | `view::{open_positions,load_with_positions}`, `decide::{project,evaluate,market_needs}`, `keeper::{markets_to_read,process_market}`. Projection calls the program prologue on copies and uses shared math/bounds. Per-market errors are recorded without skipping other markets. Reads span multiple RPC calls rather than one bank snapshot; simulation is the final program acceptance check. |
| Freshness and idle coverage | `coverage::observation_due`, `decide::market_needs` preserve no routine cadence for warmed idle inventory. The new `cadence::an_idle_market_requires_client_refresh_before_its_first_new_open` waits beyond `COVERAGE_GAP_S`, proves one crank still refuses the open, completes the next observation, then opens and observes keeper coverage start. Existing cadence/listing tests cover active positions and warm-up exceptions. |
| Exit prediction | `decide::evaluate` uses each position's locked fee cap, PumpSwap's EMA fee cap, fresh completed observations, robust spot and the program's expiry/liquidation references. A projected post-change warm-up defers normal exits; S3 keeps the shared market chain and position terms. Current restart changes flow through the shared program arming inputs. `decide` unit regressions and the liquidation/expiry/S3 scenarios pin these paths. |
| Submission and budgets | `simulate_signed`, `send_ixs_within`, `send_exit_floored`, `send_cranks` size-check, simulate, use bounded priority fees and CU headroom, distinguish failure/dry-run/landing, and retry v0 after a v1 rejection. The daily budget orders exit-critical work ahead of optional coverage; existing `budget`/`m7d` tests cover rank, tail limits, overdraft and restart funding. Multi-node read inconsistency and actual leader timing remain deployment assumptions. |
| Pauses and outages | The coverage loop tracks its own pause intent, detects foreign clears/sets, and does not treat another operator's clearance as its own. `m7_sec` and `coverage_pause` scenarios cover latency, recovery, foreign authority actions and two keepers. `redeploy` covers the loader's one-slot refusal and renewed submissions; it is distinct from a permanently missing program. |
| Upgrade monitor | `watch_programs`, `judge_inspected`, `simulate_flags`, `read_whole`, `monitor::ElfFacts`: header/flag checks precede exits, large whole-account reads follow them. A `VenueNotUpgraded` simulation only suppresses an alert when its context slot is at least the observed deploy. Batched flag simulations run concurrently. Unknown/old context and stale whole-account reads use the documented conservative paths; no live multi-node JSON-RPC concurrency test was added. |
| Pool and oracle duties | `pool_monitor::evaluate` calls the program candidate verifier and carried-fit functions; selection runs after exits/cranks. `oracle_duty` uses the same Pyth rules as the program and pages aging/refused feeds. Oracle loss falls back to Tier C rather than becoming an exit price source. Conditional pool-tier and flag-scope issues remain explicitly listed above. |
| ALT runtime | `load_lookup_tables` caches the configured table contents at startup. The tool now extends market tables correctly, but an already-running keeper must restart to load newly added addresses. Existing keys keep their indices. Missing keys are carried inline and can cause a size refusal; they do not change account identity. The restart requirement is now documented. |

AIR2-07 remains owned by the program review. Related facts: the keeper calls the program's
trimmed-hash/fingerprint functions directly; same-length deploy-slot movement always flags,
even if the prefix matches. Only account growth plus a matching recorded prefix suppresses a
flag. The 12-byte prefix is therefore a shared on-chain/keeper assumption, not an additional
keeper hash scheme. A changed fingerprint representation requires coordinated keeper handling.

## Relevant pass-2 keys and remaining operations work

- `shortwin-math|sol-settle|s3-wait-from-market-chain`: `decide::evaluate` intentionally asks
  `arming::s3_chain` using the position's own locked cap, then the market's already-observed
  chain. This is the same on-chain rule, not keeper drift. A position-specific start time would
  be an economic change; the cross-cap timing claim needs protocol-level adjudication.
- `shortwin|sol-settle|hardening-unchecked-writable-account`: the optional epoch-record account
  is checked before creation/use when an epoch rolls. A no-roll call does not read/write it;
  the proposed unconditional validation is future-use hardening, like L70/L75.
- The pass-2 liquidation/expiry fee-cap, zero-price and deny-list keys are
  program-path questions. They were surfaced to the parent review; the keeper's shared
  projection and event consumers do not independently resolve them. This document does not
  mark those program keys closed.
- The pass-2 `hardening-event-destination-hardcoded` keys for close/liquidate/expire (including
  L19, assigned during integration) are fixed: the math exposes `sweep_destination`; every
  trader sweep emitter records the pre-sweep destination and actual post-sweep wipe bits.
  AIR2-05 adds `SWEEP_TO_LENDERS = 3` for immediate credit after a dust flush. The indexer
  projection and independent replay model classify it as lender income, using the existing
  `trader_sweep_lenders` fee kind. No DB schema or event layout changed.
- AIR2-05 integration also removes the keeper's obsolete zero-share prerequisite for pending
  cleanup and lets `sweep_trader` clear the rewards bit after a dust flush with surviving shares.
  The keeper also treats a zero legacy migration marker as pending, so old source bits cannot
  suppress its one-time cleanup; an initialized deposit hold alone does not repeat the sweep.
  The real PumpSwap keeper regression settles into a dust flush, proves deposits wait, claims
  cashback, verifies immediate-credit destination/both bits, and then deposits successfully.
- Retain the V2 curve extras compatibility builder until the release after 1.0.1 has actually
  been prepared and the upgrade-window regression changes with it. This review does not infer
  permission to remove compatibility from a draft version or a future release.
  The new keeper/UI must roll out with or after program 1.0.2: an older program leaves the
  zero migration marker unchanged, so the new keeper can repeatedly pay for cleanup. V2
  trade-extras compatibility does not establish compatibility of that duty. The services
  runbook records this release gate; no deployment or capability-detection heuristic was added.
  Update the indexer first: its old destination switch misclassifies the new lender-income
  destination `3` as protocol income, while the new projection supports both `0` and `3`.
- Leave the optional deployment-slack/alert tuning and existing RPC concurrency limitations
  visible. There is no evidence here that different policy values improve production behavior.
- The migrated INGEST fixture `raydium_fast_path_venue_review.jsonl` still has an invalidated
  transaction signature. Repair requires re-recording its scenario with a valid message and
  recording procedure; fabricating a signature or relabeling old execution output would not
  repair provenance. No fixture bytes were altered in this pass.
- Keep the future-dated dependency-age exclusions and release/fuzz gates. Earlier dependency
  work upgraded rustls to 0.23.46 and zip to 9.0.5 through the licensed minimal
  `postgresql_archive` patch; no new advisory ignore was added.

## Validation

Host commands use four build jobs and release overrides `CARGO_PROFILE_RELEASE_LTO=false`
and `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`. This task did not build SBF; runtime checks use
the root task's frozen 1.0.2 release (`a6ecd6f763f9549eee5913a08d57effbcd69dd312d59217a44cc386f03e62b5e`)
and test-venue (`7449c9917d63652a7d36b9cded09a8c26137f38ce9c1513dfb7ed0d19f1505e7`) artifacts.

- Services workspace formatting and Clippy with warnings denied pass, as does the separate
  production-keeper Clippy gate. The final benchmark-source Clippy pass is also clean.
- Keeper/CLI/ops unit suites pass (92 + 1 + 17); all six sweep units pass after adding legacy
  marker cases. Real ALT extension and oracle governance regressions both pass.
- The full services workspace run passes: 409 tests, zero failures, six ignored, including all
  six doc-test targets. Keeper scenarios pass 149/149; the two ignored M7 targets pass in the
  separate benchmark run below. The other four ignored tests require a live local RPC.
- Long-gap fixtures now complete consecutive observations before opening; two sizing searches fail
  when no positive amount remains instead of looping forever on an unrelated refusal. The
  listing and cost checks pin nine PumpSwap observations, with the final bucket integrated
  on the next call. The full keeper suite covers these corrected setup paths.
- Root-executed PostgreSQL checks pass: `pg_fixture` 2/2 and `security_ingest` 12/12, including
  the new immediate lender-credit projection. The 30-day API performance/security suite also
  passes (6/6). This agent's initial network sandbox refusal was an environment limitation;
  these root-context executions provide the actual database validation.
- M7 cost, PumpSwap transaction sizes and cascade pass (3/3); the cascade alone passes again
  after refreshing its quoted cross-venue CU constants. Reports in `tests/bench/m7_keeper`
  and `tests/bench/m7_cascade` are regenerated. `keeper_cost.py` cross-checks its constants
  against the measurements, whose metadata records the final artifact hashes; all 17 generated
  ECON §5.8 tables match its output.
  The largest measured market-day increase is 0.00104 SOL; no batch-size or cascade gate fails.
- Both offline ZIP9 compatibility tests pass against the services lockfile's ZIP 9.0.5:
  generic ZIP and Zonky JAR extraction with borrowed ASCII and owned CP437 names, plus
  truncated-archive refusal. They are under `services/vendor/postgresql_archive/tests`;
  `PATCH.md` records the standalone command and synthetic-fixture provenance. The fresh
  embedded-PostgreSQL fixture separately verifies the real Zonky distribution path.

No deployment, release, mainnet probe or live RPC validation is claimed by this source-review
pass.
