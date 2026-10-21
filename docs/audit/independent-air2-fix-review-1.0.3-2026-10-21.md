# Independent AIR2 fix review — 1.0.3 — 2026-10-21

The three attributed reviews below accept the eight specific corrections AIR2-01 through AIR2-05, AIR2-08, AIR2-09 and AIR2-P2-01 within their stated evidence limits. No new blocking production defect was identified in those scopes. The accounting review found a real regression-strength gap and supplied a test-only repair; that overlap is disclosed.

This is coordinated **AI-assisted fix-review evidence** from separate agents, not an external firm's audit, a human auditor's acceptance, deployment approval or independence from the coordinating AI system. Owner acknowledgment of economic residuals is distinct from proof of these technical corrections. No aggregate “unique test” total is inferred from overlapping filtered runs, reviewer probes or repeated controls.

## Reviewed source, publication and release gates

Production identity: `e04a6776f1c2f5e02e581d76dd1255c416270275`. The AIR2-01 regression strengthening and this report/evidence publication belong to later test/documentation changes; they do not extend the production source identity. The final regression-file hash and exact production hash are recorded in the accounting section and evidence index.

These local records do not establish full candidate CI success, a completed current-profile ≥24-hour fuzz run or deployment preflight. Those final gates were pending when the reviews closed. The active frozen fuzz run was left running. See the [release-readiness record](release-readiness-1.0.3-2026-10-21.md) and [formal findings tracker](findings.md) for subsequent status, and the [owner decisions](owner-decisions-1.0.3-2026-10-21.md) for separate economic-policy acknowledgment.

The [evidence README](probes/air2-fix-review-2026-10-21/README.md), [source/retention index](probes/air2-fix-review-2026-10-21/SOURCE_INDEX.json) and [checksum index](probes/air2-fix-review-2026-10-21/SHA256SUMS) retain all three source reports byte-for-byte, small logs, check records and control/probe sources. Original `target/` execution paths quoted below identify where evidence was produced; the bundle mirrors their names/layout beneath its trackable `records/` subtree. No copied crates, binaries or large dependency tree are retained.

| Findings | Reviewing agent | Scoped conclusion |
| --- | --- | --- |
| AIR2-01, AIR2-P2-01 | `reader_fix_review` | Accept lock-rounding and unlocked-value-cap corrections; repair masked regression; preserve KI-13/KI-65 limits |
| AIR2-02, AIR2-03, AIR2-08 | `keeper_fix_review` | Accept stale-EMA, start-sample and restart-identity corrections; distinguish native probes from signed integration evidence |
| AIR2-04, AIR2-05, AIR2-09 | `remote_scope_check` | Accept protected vest/dust-source admission and raw locked-depth arithmetic; preserve availability and AMM/PnL limits |

The following sections preserve the source reports' substantive wording, measurements, commands, attribution and limitations. Only their first heading, heading levels and evidence-relative Markdown links are adapted for this consolidated location. Their unmodified originals remain separately checksummed. Fifteen logs are retained as deterministic lossless gzip to preserve their original EOF blank lines; adapted links below target those gzip files. The [compression map](probes/air2-fix-review-2026-10-21/COMPRESSION_MAP.json) maps historical raw-log references in the unchanged source reports to retained files and records both raw and compressed hashes.

## Test-only integration and separate peer review

The strengthened regression was integrated as `ff85a1b0c8452cb9fad32767f7083d22377e90a3`.
A separate `keeper_fix_review` agent inspected its direct stored/effective lock checks,
the pre-conversion cloned records and the zero-lock/zero-term control, then executed
the exact strengthened case: one passed, 15 filtered out. The reviewer accepted the
test's ability to expose the masked representation error; [raw execution log](probes/ci-fullsha-2026-10-21/air2-01-test-peer-execution.log.gz).
The isolated old-floor negative control and reviewer-authored-test overlap remain as
recorded below. This additional test review does not claim organizational independence
or replace the complete final CI and current-profile fuzz gates.

## Accounting — AIR2-01 and AIR2-P2-01

Attributed reviewer: `reader_fix_review`. [Unmodified source report](probes/air2-fix-review-2026-10-21/records/air2-independent-accounting-2026-10-21.md).

Review date: 2026-10-21. Reviewer: Codex agent `reader_fix_review`, acting separately from the author of the production accounting fixes. This is an attributed AI-assisted source and local-test review, not a review by an external audit firm.

### Source identity and independence

Reviewed production candidate: `e04a6776f1c2f5e02e581d76dd1255c416270275`. Both production fixes entered in `b815269224ba0f5d946f49de7e113220c4067041`; their conversion and allowance expressions are unchanged in the candidate. Compared their original parent formulas and the candidate control flow. Production `crates/shortwin-math/src/pool.rs` SHA-256: `3d270b521f777b314a3e86048bb5c4396bc78129d4f47eb5898942e30ad55312`.

Inputs reviewed: AI review 2 `report.md` finding 1; `pass-2-unjudged.md` L39; `triage-math-2026-10-18.md` rows AIR2-01, L39 and inherited stale-lock-era bound; `triage-pass2-2026-10-18.md`; formal `findings.md` AIR2-01/AIR2-P2-01; KI-13/KI-65; `pool::convert_steps`, `ShareEpoch::ratio`, `convert_shares`, `lock_of`, `locked_total`, `allowance`, `allowance_view`, `withdraw_in`, `deposit_in`, `resolve_in` and transactional wrappers. Also inspected the program withdrawal handler and epoch-record load/store path, and the existing accounting regression/harness.

I did not author these production math fixes or the original AIR2 accounting regressions. Earlier work in this session included frontend disclosures, VenueGuard client/readers, replay/live-venue fixtures and separate web changes. During this review I authored a **test-only strengthening** of the AIR2-01 regression after independently demonstrating a masking weakness. That addition is disclosed as reviewer-authored evidence, not a second independent author's test. No production math, program, frozen fuzz runner, fixture or service source was changed for this review.

### AIR2-01 — accept the arithmetic fix; regression weakness repaired

The old conversion floored the nominal owner lock while cumulative conversion could award that owner an additional share. The candidate uses `min(ceil(old_lock / denominator), converted_shares)`, with zero for full wipe. The validated denominator is nonzero and at most `10^36`; no unbounded product is added. A zero nominal lock stays zero. Conversion preserves the existing lock expiry and scales the calendar term downward, so it can be more conservative than aggregate locking without introducing a new expiry extension.

For a fully locked owner, cumulative converted shares are at most the ceiling of its old shares; therefore the capped ceiling lock covers every converted share. The fixture with 2,500,999 old shares and remainder 1 converts to 2,501 shares and now retains all 2,501 locked. The original report's proposed subtraction of floored unlocked shares was correctly not copied: an initially zero lock must not become a lock merely because a cumulative rounding unit is awarded.

**Independent test-strength finding:** on the unmodified candidate test, reverting only the owner lock to its old floor still passed the existing zero-allowance assertion. AIR2-P2-01's new unlocked-value cap rounds that one unlocked share's token value to zero in this fixture. This masked the AIR2-01 regression. At the parent's request I added direct stored/effective lock assertions and a zero-lock conversion control to the existing test, preserving the allowance assertion. The strengthened candidate test passes; restoring only the old floor in an isolated target-only source copy now fails with **lock 2,500 versus shares 2,501**, while the allowance fix stays enabled. This demonstrates detection of this fix's actual root cause.

**Scope/residual:** this accepts the demonstrated lock-rounding correction. I do not promote the original report's unverified larger “all free inventory” claim to an organically reproduced transaction. KI-65 conversion ordering/latent units remain separate. Ceiling versus floor can retain one additional lock unit per individual conversion before the share cap; this is a conservative share-unit difference, not a universal one-token economic bound or a proof of all conversion/lock interactions.

### AIR2-P2-01 / L39 — accept the unlocked-value cap

A dust top-up previously introduced a very small `u` while the value cap still counted all lender shares. With aggregate locks exhausting the other unlocked supply, `su = max(S - locked, u)` could equal `u`, making the free-inventory cap ineffective. The candidate caps allowance by `tokens_for_shares(pool, u)`. `withdraw_in` computes actual payout and refuses any payout above that allowance before committing lender/pool/vault/record changes. Depositing only adds its minted shares; it does not clear the previous lock.

The existing organic-operation regression is meaningful: three equal deposits, a small initial loan/withdrawal seeding `b_min`, two larger permitted loans, exhausted allowances, one repayment, then a one-token top-up. It checks the resulting 999 unlocked shares are worth zero tokens, refuses the whole-position withdrawal, and compares the subsequent loss with a no-top-up control to within one raw token. Restoring only the old whole-position value cap makes the regression fail with **allowance 670,224 instead of 0**. In the isolated old-formula source copy I also executed the actual whole-position withdrawal followed by the loss: **670,224 withdrawn, net 670,223 after the deposit, versus the control's 336,779-token residual**. This independently supports the triage's distinction from KI-13's previously disclosed +11.1% bound.

**Scope/residual:** the bound is on paid token value, not an unconditional `shares_burned <= unlocked_shares` rule. Integer-floor-equivalent or zero-token burns remain possible; they do not establish a payout beyond the unlocked-value cap. This change can reduce immediate withdrawal availability when aggregate/individual locks differ. Inherited `b_min` behavior and expiry policy remain; this review neither removes KI-13 nor re-estimates its general strategic-return bounds. No queue, payout-time guarantee, equal-return guarantee or new economic policy is inferred.

### Executed evidence

All commands ran from `/root/short.win-audit-review` with `CARGO_TARGET_DIR=/root/short.win/target`, `CARGO_BUILD_JOBS=2`, `RUST_TEST_THREADS=2` and the explicit Linux Rust PATH. The active frozen fuzz process was not signalled, restarted or used for these results. These are local host checks, not new SBF, CI or completed 24-hour fuzz evidence.

| Check | Actual result |
| --- | --- |
| `cargo test --locked -p shortwin-math --test ai2_accounting air2_01_a_fully_locked_lender_stays_locked_when_conversion_awards_a_unit -- --exact --nocapture` | Candidate strengthened test passed |
| Same command with `l39_one_token_deposit_cannot_unlock_a_fully_locked_position` | Passed |
| `cargo test --locked -p shortwin-math --lib pool::tests::convert -- --nocapture` | One matching cumulative conversion unit test passed |
| Exact `pool_econ` tests: `idle_lenders_expired_lock_stops_counting_at_lock_until`, `stuck_lock_clears_at_lock_until`, `lock_decay_restarts_with_every_lock_era` | Each passed |
| Target-only `reviewer_edges` | Four named tests passed: 53 full-lock matrix iterations using valid nonoverflowing old totals; zero-lock awarded-share controls; partial-lock/capped-to-zero/full-wipe controls; unlocked-value availability/view/expiry check |
| Isolated copy, only owner-lock floor restored | Strengthened AIR2-01 regression failed as expected, exit 101; direct full-lock matrix also failed as expected |
| Isolated copy, only whole-position value cap restored | L39 regression failed as expected, exit 101; actual old-loss replay passed its historical-value assertions |
| `rustfmt --check --edition 2024 crates/shortwin-math/tests/ai2_accounting.rs` and scoped `git diff --check` | Passed |

The negative controls are source copies under `target/air2-independent-accounting-controls-2026-10-21/`; repository production source was never patched. The driver writes copied sources with fresh mtimes before Cargo builds. A rerun initially exposed a **review-harness caching error** after timestamp-preserving copying; that diagnostic is retained as `cached-copy-diagnostic.log` and is not credited as a candidate failure. After correcting copying, candidate and negative-control executables recompiled and all expected outcomes above were reproduced. The earlier unstrengthened masked-test result remains in `floor-lock-existing-regression-masked.log`. Repeated executions and the incidental harness CRC test are not counted as additional unique protocol coverage.

Machine-readable exact commands, cwd, exit codes and per-log SHA-256 are in `target/air2-independent-accounting-controls-2026-10-21/checks.json` (SHA-256 `710082e7220a87f39838a73ed60cdc891f97b5ea55cc0209b948e473b3b0e0ef`). Reproducible driver: `target/air2-independent-accounting-controls-2026-10-21.py`. The final strengthened `crates/shortwin-math/tests/ai2_accounting.rs` SHA-256 is `1638c8b9ba4a058c283545f0d8e6588fd384369b44dee711b01d8d9bc5e2eaa4`; its pre-strengthening candidate hash was `b5b215c95e889296b078b0a5f75e2483e371001f1ce74463a7fb542e742a11f1`.

**Conclusion:** accept both scoped production fixes. The actionable test masking gap found during review has a focused, verified test-only repair ready for the parent to integrate. No additional production defect was found in these two findings. This conclusion does not close other AIR2 findings or replace pending release gates.

## Observation and settlement — AIR2-02, AIR2-03 and AIR2-08

Attributed reviewer: `keeper_fix_review`. [Unmodified source report](probes/air2-fix-review-2026-10-21/records/air2-independent-observation-2026-10-21.md).

### Identity, provenance, scope and decision

This is an AI-assisted source review and local execution record by the `keeper_fix_review` agent, requested by the repository owner through the coordinating agent. It is not an external firm's review, an official auditor's acceptance, a deployment approval or a claim of independence from the AI system coordinating the work.

Reviewed production source: `e04a6776f1c2f5e02e581d76dd1255c416270275`. The review re-read the original AIR2-02, AIR2-03 and AIR2-08 findings in `docs/audit/ai-review-2/report.md:45`, `:75` and `:350`, their math/program/pass-2 triage, and the current production control flow and regression assertions. The reviewed production files and committed regression files executed here matched that commit. Concurrent coordinator/team changes to audit documents and the unrelated `crates/shortwin-math/tests/ai2_accounting.rs` were outside this scope and were not executed by these filtered commands. Local execution finished at approximately `2026-10-20T22:35:46Z` (2026-10-21 in the owner's timezone).

**Conclusion: accept the implementation of these three specific corrections. No new blocking production defect was identified in this scope.** This is a technical fix-review recommendation, separate from owner decisions on economic risks. Full candidate CI, the new-profile ≥24-hour fuzz completion and operational preflight remain separate release gates. This record does not close other AIR2/AI3 findings or claim universal manipulation resistance.

The reviewer did not author these production instruction, observation or settlement fixes, or the committed regressions executed below. Relevant prior overlap: this reviewer authored a later keeper candidate-evaluation RPC recovery fix, legacy keeper V2 removal, a signed legacy-carry regression, and the current-schema loader-upgrade regression. None implements the three corrections reviewed here. The new native probe below was authored by this reviewer under `target/`; it is independently constructed test evidence, not independently authored production code. The already written integration report remains frozen at SHA-256 `f21bc9b9f347639b96085cd8e758b86a3e5d2bce80283f04acd7fb89021993aa`.

### AIR2-02 — stale EMA open margin

The original finding is valid: a positionless market can retain an old EMA while the pool moves, so a transient sale back toward the old EMA can satisfy the spot/EMA comparison and price the new short using stale margin inputs.

At `programs/shortwin/src/instructions/open_short.rs:272`, the instruction retains `ema_ts_before` before the prologue can integrate an old open bucket. At `:355` it requires its age to be at most `COVERAGE_GAP_S` (40 seconds). At `:362–363` it separately checks the completed observation's `lb_sec` when that observation exists. The latter matters because a preceding crank, including one earlier in the same transaction, may update `ema_ts` while integrating an old bucket; the integration timestamp alone is insufficient. The checks run before the margin/cap calculations. A failed transaction cannot leave the attempted open or its prologue changes committed.

Executed the complete focused `ai2_stale_ema_it` binary: **4 passed, 0 failed**.

- The attack scenario uses actual mock-venue swaps, an idle market with no open position, an EMA age of 60 seconds, and live spot sold back to 14 bps from the stale EMA. The signed open is refused with custom error `6101` (`EmaWarmingUp`).
- Boundary control: an age of 40 seconds is permitted and 41 seconds is refused.
- A same-transaction `[crank_price, open_short]` with an old bucket is refused. A separate old-bucket crank advances `ema_ts` but preserves the old `lb_sec`, and a subsequent open remains refused. Recording a current sample and integrating it in the next second restores ordinary opening.
- The fourth test exercises conservative first-touch restart-marker initialization, discussed under AIR2-08.

The mock isolates shared production open-guard code using the test-venue SBF. It does not independently demonstrate the complete attack economics on every external venue. The test's initial whale/attacker inventories are funded by the harness; this is not an attacker net-profit proof. The accepted correction is the explicit freshness gate and demonstrated old-bucket bypass prevention, not a claim that any price younger than 40 seconds is unmanipulable or that a positionless market always receives paid keeper observations.

**Disposition: implementation accepted for the reported stale-EMA defect.**

### AIR2-03 — PumpSwap start sample pins the warm-up seed

The original finding is valid: when the same briefly depressed spot both initializes the EMA and enters the first warm-up bucket, the warm-up's running minimum retains that initial low through the eight integrations.

At `programs/shortwin/src/engine/prologue.rs:198–206`, `start_price` returns whether this instruction initialized the PumpSwap price. Only the non-starting path calls `observe::record`. The starting path still creates the initial ring/EMA state and writes the restart identity; it leaves the start sample out of the subsequent observation bucket. `programs/shortwin/src/venues/pump_amm.rs:876–892` preserves the previous observation count when initializing a previously unpriced state. Later executable observations supply the warm-up buckets.

Executed `ai2_pumpswap_start_it`: **1 passed, 0 failed**, using the frozen production short.win ELF and the new CURRENT Pump/PumpSwap fixture profile.

The real signed atomic transaction sells `2·X0` base tokens, starts the price with a crank, then buys those tokens back on the dumped PumpSwap ELF. The precondition asserts no price state before the transaction, live lender shares, a pushed start below half the restored price, and a restart marked with eight warm-up integrations.

Actual output:

| Measurement | Value |
|---|---:|
| Restored pool spot, WAD-scaled | 48,416,447,748,031,702 |
| Pushed start EMA, WAD-scaled | 5,401,123,726,570,019 |
| EMA at warm-up gate, WAD-scaled | 20,429,424,038,941,487 |
| Cranks to gate | 9 |
| Gate EMA / restored spot, integer percent | 42% |
| Later signed open collateral | 24,243,736,590 lamports |
| Later signed open live buyback cost | 8,012,774,276 lamports |

The gate assertion at `tests/litesvm/tests/ai2_pumpswap_start_it.rs:154` specifically proves that the EMA is no longer pinned to the start value. It does **not** assert complete convergence to the restored spot at the gate. The subsequent signed open waits through the normal 50-second listing window (`:165–170`), then checks collateral at least 150% of live buyback cost (`:201`); the observed ratio exceeds 3×.

The test prints the hypothetical sentence `a short opened here is under-margined by ~2x` at the warm-up gate. It does not execute such an open there. That line must not be cited as an executed harmful position or evidence that the fixed candidate permits that opening: listing and spot/EMA guards still apply. The actual executed open is the later adequately collateralized control above. The fixture supplies the attacker's starting inventory; this is not a measured profitable attack or a universal warm-up security bound. Repeated manipulated observations and other already documented degraded modes are outside this specific correction.

**Disposition: implementation accepted for start-sample pinning, with the stated convergence/evidence limits.**

### AIR2-08 — pre-restart settlement arms reuse old evidence

The original finding is valid. Eight warm-up integrations can fit inside the 10-second arm-call gap. Merely suppressing settlement while warming, or clearing an arm only when a settlement call sees warm-up, cannot handle a caller that evaluates only after the gate.

The current solution persists a restart identity rather than relying on having observed the gate:

- `programs/shortwin/src/state/market.rs:660–664` stores the observation count at restart **plus one**, using checked addition. This invalidates an arm whose count equals the restart count as well as older arms. `restart_ema_count` occupies reserved Market storage (`state/market.rs:470`; `state/layout_asserts.rs:177`), without enlarging Market.
- New market initialization sets a nonzero marker (`instructions/create_market.rs:662`, with the Raydium initialization path at `:678`). Prologue first-touch handling initializes a zero legacy marker once before evaluation (`engine/prologue.rs:165–168`). The owner's never-deployed confirmation means there is no present migration prerequisite; the conservative first-touch behavior remains a reviewed conditional code path.
- Source review found marker writes at both Raydium adoption branches (`engine/pool_select.rs:610,615`), curve-to-PumpSwap transition and PumpSwap price start (`engine/prologue.rs:192,201`), and the curve/PumpSwap re-detection paths (`instructions/crank_price.rs:225,260`). Previously unpriced initialization preserves `ema_count` in `pump_amm::start_price`, the Raydium adoption path and curve re-detection. Thus a new marker is not compared against a silently reset counter.
- `engine/arming.rs:244` passes the persisted marker into the sole production `SettleInputs` construction. `instructions/sol_settle.rs:220–281` obtains post-prologue market/observation state, skips evaluation on a graduation instruction whose ReadSet is still the old curve's, and otherwise calls the math evaluator.
- Both live-clock checks require `state.count >= inp.restart_ema_count`: the non-liquidatable branch (`crates/shortwin-math/src/settle.rs:598`) cannot refresh an old arm's last-call time, and the shared S1/S2 branch (`:610`) cannot reuse its old count. Once qualifying fresh evidence returns, it re-arms at current `ema_count` and waits the full persistence interval. Warm-up still blocks executable-venue evaluation (`:581`); the restart marker does not replace or disable the independent S3 route.

Executed evidence:

1. Existing native regression `settle::tests::air2_08_a_restart_rearms_even_when_no_call_observed_the_warmup`: **1 passed**. It evaluates only after the eight warm-up integrations at time 109, within the arm gap from time 100, with old arm counts 60 and 100 and restart marker 101. Both re-arm at count 108 and settle only after 40 additional integrations. Its toy quotes select S1.
2. Independently authored isolated native probe: **2 passed**. The first test explicitly exercises **both S1 and S2**, each with an older arm and an equal-at-restart arm; all four re-arm and wait 40 new updates. The same inputs with the marker disabled retain the old clock, with the older-count arm settling immediately, establishing sensitivity to restart identity. The second test separately covers the non-liquidatable branch and the no-fresh-spot branch: neither refreshes a pre-restart clock; marker-disabled controls do refresh it. This probe uses public math APIs and constant test quotes, not real external venue quotes.
3. Existing signed `switch_confirm_it::the_warm_up_gates_keeper_exits_and_settlements`: **1 passed**, 5 filtered. A real fast pool adoption leaves eight seconds of refused keeper exits and no-op settlement evaluations, then liquidation succeeds at +9 seconds. The harness I18 hook (`tests/litesvm/src/invariants/confirm.rs:76–84`) independently asserts that the restart preserves the count and persists `post.ema_count + 1`. This test starts unarmed: it verifies actual restart persistence and warm-up control flow, not the complete old-armed missed-warm-up scenario.
4. The signed first-touch case in `ai2_stale_ema_it` resets only the newly used reserved marker to zero, then proves one crank initializes it to `old_count + 1` and the next crank does not move it. It does not seed an armed position or settle it.

Together the source wiring, actual restart/first-touch transactions, and focused evaluator controls support accepting this correction. An all-in-one signed regression that arms a position, performs a real restart, skips all settlement calls during warm-up, then demonstrates 40 new updates would strengthen integration coverage further. That exact all-in-one path was not executed in this review, and the existing native regression alone must not be described as on-chain settlement evidence.

**Disposition: implementation accepted for pre-restart clock reuse; no general settlement liveness or payout guarantee is implied.**

### Executed commands and immutable inputs

Working directory: `/root/short.win-audit-review`. Every Cargo command used:

```sh
export PATH=/root/.cargo/bin:/usr/local/bin:/usr/bin:/bin
export CARGO_BUILD_JOBS=2
export CARGO_TARGET_DIR=/root/short.win/target
export SHORTWIN_SO=/root/fuzz/ready-103-20261021-e04a6776/bin/shortwin-release.so
export SHORTWIN_TEST_VENUE_SO=/root/fuzz/ready-103-20261021-e04a6776/bin/shortwin-test-venue.so
export MOCK_CPAMM_SO=/root/fuzz/ready-103-20261021-e04a6776/bin/mock_cpamm.so
export SHORTWIN_FIXTURES=/root/fuzz/ready-103-20261021-e04a6776/fixtures
```

```sh
cargo test -p shortwin-math --lib air2_08 --locked -- --nocapture
cargo test -p shortwin-litesvm-tests --test ai2_stale_ema_it --locked -- --test-threads=2 --nocapture
cargo test -p shortwin-litesvm-tests --test ai2_pumpswap_start_it --locked -- --test-threads=2 --nocapture
cargo test -p shortwin-litesvm-tests --test switch_confirm_it the_warm_up_gates_keeper_exits_and_settlements --locked -- --exact --nocapture
cargo test --manifest-path target/air2-independent-native-probe/Cargo.toml --offline --locked -- --nocapture
```

Commands were logged with `set -o pipefail` and `2>&1 | tee <log>`. An initial invocation of the native AIR2-08 regression passed its test but had a malformed PowerShell/bash post-test exit-status check; it was rerun with the corrected pipefail command above and exited successfully. The first independent probe compilation lacked two imports (`Budget`, `SettleKind`); only the untracked probe was corrected before the successful run. Neither was a production failure.

The SBF paths were independently hashed for this review:

| Artifact | SHA-256 |
|---|---|
| Frozen release | `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc` |
| Frozen test venue | `0a405deba60a521798820985cd4cec1a0e61e46ed9d0ee829fd037b010565a01` |
| Frozen mock | `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9` |

The new CURRENT fixture identities and candidate/source-archive consistency were independently verified in the frozen [integration review](independent-integration-review-1.0.3-2026-10-21.md); these tests selected that same frozen fixture directory. No broad fixture revalidation or duplicated full suite was run for this task.

Local result/probe artifacts retained under `target/`:

| File | SHA-256 |
|---|---|
| `air2-independent-stale-2026-10-21.log` | `d75c7f4aeabaa048e33e3d3d953fb86de4f564aa0ca0ce37a937aaf3314f20d2` |
| `air2-independent-pumpswap-2026-10-21.log` | `06213a17e7e5edcbbeb5078e7028a6781ef72a1890fdaf8761dc3bf094fe6587` |
| `air2-independent-settle-2026-10-21.log` | `a6281795657d99a977b11010e471bb029cf6641c8910d943b909bfb154fc0baf` |
| `air2-independent-switch-2026-10-21.log` | `98749ec77e83c6e6cf394be827e9cedc2fda46aab40a5e97a0876b0cba3b1256` |
| `air2-independent-extra-native-2026-10-21.log` | `07e8e1d0c9ba4f141af7b3a1ad909158a1073f0f51204405cc7a6998e519e700` |
| `air2-independent-native-probe/Cargo.toml` | `6f24e5c380e981d774afef614e46ee7c08c3dc32449b4a594efa5f1a88a95757` |
| `air2-independent-native-probe/Cargo.lock` | `00f0d07a68d17f2219641bda214cb96a6664e587860adc852b0d0cd9e75f2e7a` |
| `air2-independent-native-probe/src/lib.rs` | `cc715380e4b7be3a003436a3a7ec352c92961a3f5613afc4289a1f1688183e1c` |

No production, committed test, fixture, original audit archive or previously frozen integration-review file was edited. No commit was made. Fuzzer PID `150247` was read-only inspected and left running; this review does not claim ≥24-hour completion from the current partial run.

## Vesting and locked depth — AIR2-04, AIR2-05 and AIR2-09

Attributed reviewer: `remote_scope_check`. [Unmodified source report](probes/air2-fix-review-2026-10-21/records/air2-independent-vesting-depth-2026-10-21.md).

2026-10-21. Reviewed source: `e04a6776f1c2f5e02e581d76dd1255c416270275`.

This is an independent AI source review and focused local execution record for AIR2-04, AIR2-05 and AIR2-09. This reviewer did not author these production fixes. Earlier work by this reviewer covered the full-digest guard, the FR01 Python deposit-guard mirror, service TLS and an indexer event regression. Those are separate scopes. The original findings and triage identified the mechanisms; current code and executable regressions determined the conclusions. No external firm or human audit is represented.

Recommendation: **addressed for the three original technical mechanisms in the reviewed scope**, with the limits below. An economic owner acknowledgment is not used as proof of a technical fix.

### AIR2-04

Original finding 4 (`docs/audit/ai-review-2/report.md:107`) describes new shares capturing the vested SOL paid for tokens extinguished by a halted-venue S3. Its proposed half-claims threshold also permits losses to be split across smaller settlements.

Current `pool::settle` sets `hold` whenever a non-flushing settlement leaves both shares and a live vest (`crates/shortwin-math/src/pool.rs:2437`), regardless of that individual settlement's loss fraction. Its sole production caller, `instructions/sol_settle.rs:323-328`, records the marker after removing the position, including the halted-venue case. `instructions/deposit.rs:130-135` first accrues/releases income, then requires actual shared `vest_bal == 0` while the hold marker is armed. Passing the initially recorded end time is insufficient. A successful post-release deposit resets the marker to -1.

Fresh markets initialize -1 (`create_market.rs:661`). Zero is the legacy encoding: `engine/prologue.rs:90` invalidates old source bits and arms the marker before release. The regression's legacy encoding is explicitly synthetic. The user states that the program has never been deployed; no migration requirement is inferred here.

Actual signed execution: `ai2_halted_s3_vest_it` passed on the release ELF below. It reaches S3 using real Raydium buys and the admin's real SetParams halt, then forks identical pre-settlement VM state into control and late-depositor branches. The late deposit is refused with PoolDrained. The settlement extinguishes 3,149,121,988 raw tokens and leaves 11,711,752,240 lamports vested. The original holder receives 11,711,842,813 lamports in both branches; the late depositor receives zero. The test also verifies entry after release, a zero-marker control, required cleanup after that control, and refusal at the original end after a later sweep extends the shared vest.

Math regressions additionally passed for smaller and larger settlements, a full-wipe flush without a hold, and five individually sub-threshold settlements leaving a combined 5 SOL vest. This covers the split-settlement bypass separately.

Conclusion: the demonstrated late-deposit capture and split-settlement bypass are prevented. This concerns settlement-protected historical income; the existing ordinary-income policy remains separate.

### AIR2-05

Original finding 5 (`report.md:180`) describes a dust flush retaining shares while cashback remains outside the pool. A new deposit could enter at the collapsed token price before old sources were booked.

`pool::settle` clears `wipe_swept` on every flush (`pool.rs:2433`), including one leaving shares. `deposit_in` requires Trader and, when `rewards_required`, VenueRewards source bits even when total_shares remains positive (`pool.rs:2050-2056`), before committing new shares.

`sweep_destination` (`pool.rs:377`) chooses Lenders for a pending source with surviving shares. `pool::sweep` (`pool.rs:2216`) selects that destination before setting the bit and immediately credits that first source; it does not re-vest it ahead of admission. Already-cleared ordinary source income still vests.

`instructions/sweep_trader.rs:248-264` binds supplied account keys to the market's cached trader/UVA keys and checks the pump UVA and PumpSwap UVA WSOL ATA for remaining claimable value. It marks VenueRewards complete only after those checks report clear (`371-383`). `engine/lender.rs:202` also requires rewards for markets retaining a pump UVA after leaving a pump venue.

Split claims require a second defense: an empty Trader sweep can set that source bit before a later cashback claim. A later claim arrives through Trader again and may vest. `engine/prologue.rs:96` arms the deposit hold while reward cleanup is pending and shares survive. The marker remains armed when the later sweep clears the reward bit, so the actual-vest deposit gate still prevents capture.

Actual math execution passed the dust-flush capture regression: both sources are required; 1 SOL of historical rewards is credited before admission; the newcomer receives zero; old holders receive approximately 51 SOL. The ordinary-income control passed separately: a later equal-sized lender may receive 0.5 SOL of an ordinary 1 SOL vest.

Actual signed execution passed both AIR2-05 cases in `ai2_venue_review_it`: organic dust flush with surviving shares and outstanding PumpSwap cashback; initial empty Trader sweep; separate real cashback claim and later sweep that re-vests income; refused late deposit until release; zero cashback paid to the newcomer after admission. The zero-marker/stale-source-bit control passed too.

Conclusion: the original dust-flush capture and split-claim variation are prevented for the checked sources. This does not establish recovery of cashback in an external noncanonical account or claim availability when upstream execution is unavailable. Those distinct reward-availability issues retain separate dispositions.

### AIR2-09

Original finding 9 (`report.md:418`) concerns premature whole-bps flooring. Counter/supply/equal-reserves 10,000/9,999/10,000 gave D=1; a proportional unlocked deposit changing them to 10,001/10,000/10,001 made the old formula return zero.

`pool_select::locked_depth` (`crates/shortwin-math/src/pool_select.rs:41`) now computes `floor((counter-supply)*isqrt(X*Y)/counter)` directly. lambda_bps remains a display/threshold quantity. Zero counter or supply above counter returns zero; zero reserves return zero. Two u64 reserves fit in the u128 product, and the raw locked numerator cannot exceed the counter, keeping D <= isqrt(X*Y).

Production callers use the shared helper on checked venue state: Raydium v4 (`venues/raydium_v4.rs:182`), CPMM (`raydium_cpmm.rs:240`) and PumpSwap (`pump_amm.rs:739-743`). The Raydium LP reader requires an exact 82-byte Tokenkeg mint; the venue readers bind mint/vault keys to the pool. Depth refresh, eligibility and adoption consume the resulting locked_depth. Unrelated asynchronous keeper ranking is not certified by this arithmetic review.

The normative model defaults to the same raw ratio (`docs/design/sim/shortwin_sim.py:239`, methods at 685 and 796). Historical attack drivers may explicitly select the former bps mode.

Actual tests passed the exact counterexample, scaled-reserve cases, maximum-u64 arithmetic, raw-ratio/conservative comparisons and invalid-counter boundaries. Two focused properties ran with at least 2,000 cases each: the BigUint raw-ratio comparison, and the reference-depth property using the standard integer square root. The latter bounds proportional deposits to one depth unit and checks a stated reserve-rounding bound on withdrawals.

Actual signed venue tests passed: real CPMM nine-times unlocked deposit, adoption and withdrawal preserve D within the regression tolerance; PumpSwap three-times deposit/withdrawal changes lambda from 10,000 to 2,500 bps, scales square-root liquidity fourfold, and reports depth ratio 1.000000 while preserving sizing/tier/reference bounds. These integration tests allow one-bp tolerance; the narrow math cases/properties address the integer-edge defect.

Conclusion: premature-bps rounding is addressed. Proportional invariance assumes coherent counter/supply/reserves and proportional LP changes. Reserve rounding, retained fees and real AMM-v4 PnL recognition are separate effects. L74 is not closed by this review; no real AMM-v4 deposit against target-orders/PnL state was executed here.

### Scope and availability

AI3-L10's acknowledged renewable deposit denial is distinct from technical capture prevention. Actual-vest isolation remains required even when donations prolong the hold. This review promises no universal reopening time or total cost/duration bound. Existing-owner claim/withdraw rules and the separately recorded economic choice remain unchanged.

No production source, lockfile, fixture or fuzzer input was edited. Focused native builds and evidence files used target/cache directories. This review used existing hash-checked SBF artifacts; it did not rebuild SBF, run full CI, finish the active fuzz gate or establish deployed state.

### Executed checks

Working directory `/root/short.win-audit-review`. Environment: `CARGO_BUILD_JOBS=2`, verified shared `CARGO_TARGET_DIR=/root/short.win/target`, `PROPTEST_CASES=2000`.

Runtime paths were passed explicitly through SHORTWIN_SO, SHORTWIN_TEST_VENUE_SO and MOCK_CPAMM_SO:

- `target/deploy/shortwin.so`: `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc`.
- `target/test-venue/deploy/shortwin.so`: `0a405deba60a521798820985cd4cec1a0e61e46ed9d0ee829fd037b010565a01`.
- `target/deploy/mock_cpamm.so`: `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9`.

PumpSwap tests use the current Oct2026QuoteFees profile. The dumped pump/PumpSwap ELF files hash respectively to `05249b60f8ddf02a53d0f8a43980fde40cce33f73526355b4ef08e292279819f` and `51a5f6a7fb5a812de7686e593ce6159f6ee59b15771a3d5d37ab661c46d64586`.

| Exact command | Result | Log |
|---|---|---|
| `cargo test -p shortwin-math --test ai2_accounting air2_05 --locked` | test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s | [log](probes/air2-fix-review-2026-10-21/records/air2-independent-vesting-depth/math-air2-05.log.gz) |
| `cargo test -p shortwin-math --test ai2_accounting air2_09 --locked` | test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s | [log](probes/air2-fix-review-2026-10-21/records/air2-independent-vesting-depth/math-air2-09.log.gz) |
| `cargo test -p shortwin-math --test ai2_accounting pass2_split_settlements_cannot_avoid_the_live_vest_hold --locked` | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.00s | [log](probes/air2-fix-review-2026-10-21/records/air2-independent-vesting-depth/math-split-settle.log.gz) |
| `cargo test -p shortwin-math --lib a_settlement_marks_any_remaining_live_vest_for_a_deposit_hold --locked` | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 146 filtered out; finished in 0.00s | [log](probes/air2-fix-review-2026-10-21/records/air2-independent-vesting-depth/math-vest-hold.log.gz) |
| `cargo test -p shortwin-math --test quotes_props locked_depth_properties --locked` | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 21 filtered out; finished in 0.04s | [log](probes/air2-fix-review-2026-10-21/records/air2-independent-vesting-depth/math-depth-properties.log.gz) |
| `cargo test -p shortwin-math --test oracle_props the_depth_cap_bounds_deposit_inflation --locked` | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.04s | [log](probes/air2-fix-review-2026-10-21/records/air2-independent-vesting-depth/math-depth-cap.log.gz) |
| `cargo test -p shortwin-litesvm-tests --test ai2_halted_s3_vest_it --locked -- --nocapture` | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.94s | [log](probes/air2-fix-review-2026-10-21/records/air2-independent-vesting-depth/svm-halted-s3.log.gz) |
| `cargo test -p shortwin-litesvm-tests --test ai2_venue_review_it air2_05 --locked -- --nocapture` | test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 9.63s | [log](probes/air2-fix-review-2026-10-21/records/air2-independent-vesting-depth/svm-dust-split.log.gz) |
| `cargo test -p shortwin-litesvm-tests --test pool_attacks_it unlocked_liquidity_moves_lambda_not_d --locked -- --nocapture` | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 16 filtered out; finished in 1.09s | [log](probes/air2-fix-review-2026-10-21/records/air2-independent-vesting-depth/svm-cpmm-lp.log.gz) |
| `cargo test -p shortwin-litesvm-tests --test pamm_it a_deposit_and_its_withdrawal_move_neither_d_nor_the_bounds --locked -- --nocapture` | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 78 filtered out; finished in 8.22s | [log](probes/air2-fix-review-2026-10-21/records/air2-independent-vesting-depth/svm-pamm-lp.log.gz) |

All 10 commands returned exit 0: 8 focused math tests (including 2 properties) and 5 signed SVM tests. The machine record [results.json](probes/air2-fix-review-2026-10-21/records/air2-independent-vesting-depth/results.json) includes exact commands, environment, return codes, log SHA256 values and runtime artifact hashes. Its SHA256 is `15b4707b6f6655a0d0005e6fac1475b2fe6976ec9a51318c82fd9d4a2cce1587`.
