# Independent focused AIR2 observation and settlement review — 2026-10-21

## Identity, provenance, scope and decision

This is an AI-assisted source review and local execution record by the `keeper_fix_review` agent, requested by the repository owner through the coordinating agent. It is not an external firm's review, an official auditor's acceptance, a deployment approval or a claim of independence from the AI system coordinating the work.

Reviewed production source: `e04a6776f1c2f5e02e581d76dd1255c416270275`. The review re-read the original AIR2-02, AIR2-03 and AIR2-08 findings in `docs/audit/ai-review-2/report.md:45`, `:75` and `:350`, their math/program/pass-2 triage, and the current production control flow and regression assertions. The reviewed production files and committed regression files executed here matched that commit. Concurrent coordinator/team changes to audit documents and the unrelated `crates/shortwin-math/tests/ai2_accounting.rs` were outside this scope and were not executed by these filtered commands. Local execution finished at approximately `2026-10-20T22:35:46Z` (2026-10-21 in the owner's timezone).

**Conclusion: accept the implementation of these three specific corrections. No new blocking production defect was identified in this scope.** This is a technical fix-review recommendation, separate from owner decisions on economic risks. Full candidate CI, the new-profile ≥24-hour fuzz completion and operational preflight remain separate release gates. This record does not close other AIR2/AI3 findings or claim universal manipulation resistance.

The reviewer did not author these production instruction, observation or settlement fixes, or the committed regressions executed below. Relevant prior overlap: this reviewer authored a later keeper candidate-evaluation RPC recovery fix, legacy keeper V2 removal, a signed legacy-carry regression, and the current-schema loader-upgrade regression. None implements the three corrections reviewed here. The new native probe below was authored by this reviewer under `target/`; it is independently constructed test evidence, not independently authored production code. The already written integration report remains frozen at SHA-256 `f21bc9b9f347639b96085cd8e758b86a3e5d2bce80283f04acd7fb89021993aa`.

## AIR2-02 — stale EMA open margin

The original finding is valid: a positionless market can retain an old EMA while the pool moves, so a transient sale back toward the old EMA can satisfy the spot/EMA comparison and price the new short using stale margin inputs.

At `programs/shortwin/src/instructions/open_short.rs:272`, the instruction retains `ema_ts_before` before the prologue can integrate an old open bucket. At `:355` it requires its age to be at most `COVERAGE_GAP_S` (40 seconds). At `:362–363` it separately checks the completed observation's `lb_sec` when that observation exists. The latter matters because a preceding crank, including one earlier in the same transaction, may update `ema_ts` while integrating an old bucket; the integration timestamp alone is insufficient. The checks run before the margin/cap calculations. A failed transaction cannot leave the attempted open or its prologue changes committed.

Executed the complete focused `ai2_stale_ema_it` binary: **4 passed, 0 failed**.

- The attack scenario uses actual mock-venue swaps, an idle market with no open position, an EMA age of 60 seconds, and live spot sold back to 14 bps from the stale EMA. The signed open is refused with custom error `6101` (`EmaWarmingUp`).
- Boundary control: an age of 40 seconds is permitted and 41 seconds is refused.
- A same-transaction `[crank_price, open_short]` with an old bucket is refused. A separate old-bucket crank advances `ema_ts` but preserves the old `lb_sec`, and a subsequent open remains refused. Recording a current sample and integrating it in the next second restores ordinary opening.
- The fourth test exercises conservative first-touch restart-marker initialization, discussed under AIR2-08.

The mock isolates shared production open-guard code using the test-venue SBF. It does not independently demonstrate the complete attack economics on every external venue. The test's initial whale/attacker inventories are funded by the harness; this is not an attacker net-profit proof. The accepted correction is the explicit freshness gate and demonstrated old-bucket bypass prevention, not a claim that any price younger than 40 seconds is unmanipulable or that a positionless market always receives paid keeper observations.

**Disposition: implementation accepted for the reported stale-EMA defect.**

## AIR2-03 — PumpSwap start sample pins the warm-up seed

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

## AIR2-08 — pre-restart settlement arms reuse old evidence

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

## Executed commands and immutable inputs

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

The new CURRENT fixture identities and candidate/source-archive consistency were independently verified in the frozen [integration review](../docs/audit/independent-integration-review-1.0.3-2026-10-21.md); these tests selected that same frozen fixture directory. No broad fixture revalidation or duplicated full suite was run for this task.

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
