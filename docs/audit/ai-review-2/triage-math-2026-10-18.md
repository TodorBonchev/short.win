# AI review 2: math and accounting triage, 2026-10-18

This is a source review and local host-test record for AIR2-01, AIR2-05, AIR2-08,
AIR2-09, AIR2-13; leads L15, L16, L30, L38, L39, L40, L41, L43, L52, L74, L79;
and the related math keys in `pass-2-unjudged.md`. The numbers refer to the numbered
findings and the order of the lead bullets in `report.md`. The report's proposed
patches are hypotheses, not evidence. Existing owner decisions in
`docs/audit/known-issues.md` were checked against each mechanism and its bound.

Changes below are local draft changes. Host tests do not establish a deployed fix,
an organic venue exploit, or canonical SBF validation. The parent task owns the
final SBF build, migration integration and transaction-level checks. No historical
audit result is being silently reclassified as a measurement of these changes.

## Findings and assigned leads

| Item | Disposition | Evidence, scope and remaining work |
|---|---|---|
| AIR2-01: converted lock rounds down | **Confirmed arithmetic bug; fixed locally.** | Conversion may award a share through `conv_rem` while flooring its owner's lock. A valid post-rebase fixture had 2,500,999 old shares, all locked, denominator 1,000, remainder 1: 2,501 shares but only 2,500 locked, allowing one raw token. Conversion now uses `min(ceil(old_lock / denominator), converted_shares)`; a full wipe gives zero. `air2_01_a_fully_locked_lender_stays_locked_when_conversion_awards_a_unit` now permits zero. The report's patch `new_shares - floor(old_unlocked / denominator)` was not copied: it can create a lock where the old nominal lock was zero. This fixes the demonstrated rounding mismatch; the report's larger claim that it takes **all** free inventory was not separately established through an organic rebase transaction. KI-65's owner-conversion ordering and latent-unit residual remain distinct. |
| AIR2-05: rewards after dust flush go to new depositors | **Confirmed accounting bug; fixed locally.** | Two lenders deposit 10^12 tokens each, borrow 90%, exhaust their allowances, and leave one raw token. Settling the loan with 50 SOL flushes the vest while shares remain. Before the fix a new 1,000,000-token deposit was accepted and minted 900,499,500,000 shares ahead of old rewards. Every settlement flush now clears the source sweep bits; deposits require them even when `S > 0`. The first pending-source sweep credits the surviving holders immediately instead of starting another vest. The regression requires both trader and venue-reward sources, credits 1 SOL of old rewards, then allows the deposit: the new lender receives zero and old holders receive approximately 51 SOL. A separate normal-income regression preserves the existing behavior: a normal 1 SOL sweep vests and a later equal-sized deposit earns 0.5 SOL after six hours. Program event routing, reward-clear verification, keeper and indexer integration are owned by the services triage. |
| AIR2-08: arming survives restart | **Confirmed persistence bug; fixed locally with program integration required.** | Warm-up alone does not identify a restart when no evaluation occurred during it: eight warm-up integrations can fit inside the ten-second arm-call gap. Both S1 and S2 live-arm checks now additionally require `arm.count >= restart_ema_count`. The marker is the observation count **at restart plus one**; zero means no recorded marker. The plus one invalidates an old arm whose count equals the restart count, as well as older arms. The regression checks both cases, evaluates only after warm-up, requires rearming then, and requires all 40 new integrations. Root adds the persisted market marker in reserved storage, records it at restart sites, preserves the count across absent-price initialization, and invalidates legacy arms on first touch. Merely clearing arms in the warm-up branch would not fix the missed-call case. |
| AIR2-09: bps floor destroys deposit invariance | **Confirmed arithmetic/design contradiction; fixed locally with approved normative change.** | With counter 10,000, supply 9,999 and equal reserves 10,000, `D=1`. A proportional unlocked deposit of one makes counter 10,001, supply 10,000 and reserves 10,001: the old whole-bps formula gives `D=0`. Scaling reserves reproduces a 10 SOL depth change. `locked_depth` now computes `floor((counter-supply)*isqrt(X*Y)/counter)` directly; zero counter or supply above counter still gives zero. `lambda_bps` remains a display helper. The old error is at most `ceil(isqrt(X*Y)/10,000)` in absolute depth, but can be 100% of small locked depth; there is no useful relative bound at that edge. New properties independently compare raw-ratio arithmetic, bound proportional-deposit change to one integer unit, and check reserve-rounding bounds on withdrawal. Parent explicitly approved changing the previous whole-bps normative rule. This does not resolve L74's real fee-recognition effect. |
| AIR2-13: rent can consume keeper floor | **Condition confirmed; keeper liveness remains an open economic issue.** | `V=20 SOL`, buyback `B=19 SOL`, rent due `1 SOL` pays all remaining SOL as rent and pays the liquidation keeper zero despite the nominal `K_s=0.21 SOL`. `air2_13_rent_exhaustion_pays_no_keeper_even_with_a_floor` pins it. The waterfall and N22 pay the keeper floor only from remaining borrower equity; KI-03 discloses that liquidations reserve nothing, but is itself **disclosed**, not a blanket acceptance of guaranteed keeper service. No accounting imbalance occurs. A profitable attacker strategy, frequency under deployed terms, or inability of the in-house keeper to serve this case was not established here. Changing priority/reserves would change accepted economic rules and requires an owner decision; no such change was made. |
| L15: raw clock in claim/cover/sync | **Standalone future-check allegation unproven; related clock mismatch hardened.** | The claimed future comparison to `vest_last` is not in today's claim path. However, read-only-market lender operations can advance the vault calendar without advancing the market accrual timestamp; L16/L79 demonstrate the relevant inconsistency. The report asserts current Agave prevents clock regressions; this review does not establish a mainnet clock-regression path. |
| L16: claim advances calendar beyond accrual time | **Conditional correctness bug reproduced; fixed locally.** | A lender touch advances the calendar to the lock's expiry; a subsequent timestamp one second earlier previously resurrected a nominal lock whose aggregate calendar bucket was already removed. A withdrawal could then add a term to a dead bucket and fail. `lock_of` now treats `day(lock_until) < vault.cal_day` as expired, independent of the raw clock. The regression advances the calendar through the other lender's pot-only touch, steps the clock back, and verifies the old lock stays zero and a withdrawal succeeds. This is an internal monotonicity fix; the synthetic clock step is explicit. |
| L30: observation gap cancels a collection window | **Covered continuity rule, not a demonstrated bypass.** | `chain_step` preserves the chain at exactly 60 seconds and restarts it at 61; the focused boundary test pins both. `window_chain_holds` requires the failing-route chain to predate the window, so a reset legitimately loses the evidence needed to adopt. A configured 120-second window therefore needs observations inside it, as do default 60-second windows executed late. Existing fast-path tests document the requirement; normal keeper cadence is 30 seconds. The lead describes a keeper outage/continuity failure, not a window that still has valid continuous evidence. No rule was relaxed. |
| L38: token account accepted as LP mint | **Decoder hardening; no reachable account substitution found.** | `mint::read_base` accepts buffers at least 82 bytes, so a crafted 165-byte token-account buffer can resemble base mint fields. Both Raydium adapters bind the LP account key to the venue-owned pool's LP mint and the cached read set; an attacker-created token account cannot simply replace that key. Root adds exact SPL mint length validation in the program helper. This strengthens the boundary without claiming a demonstrated depth attack. |
| L39: locked shares count in allowance's value cap | **New pass-2 bound violation confirmed; fixed locally. Not covered by KI-13.** | Three lenders deposit 1,000,000 tokens each. A 10,000-token loan and small first withdrawal seed a low `b_min`; two 1,000,000-token loans pass the 90% utilization cap. All lenders exhaust their allowances and become fully locked. Repaying one large loan restores 1,000,000 free tokens but no allowance. A one-token deposit mints 999 unlocked shares, worth zero tokens, yet the old formula permits withdrawing the whole 670,224-token position. After the other large loan loses all principal, the no-deposit control retains 336,779 tokens: the runner's net 670,223 is about 99% above the control, beyond KI-13's +11.1% disclosed run bound. The pre-fix run passed locally. The approved fix caps allowance at `tokens_for_shares(pool, unlocked_shares)` instead of the value of all shares. The regression pins the old cap, rejects the full withdrawal after the tiny deposit, and preserves the control residual to one token. **Availability changes:** stale aggregate locks may now permit fewer free-token withdrawals; a deposit cannot use its tiny unlocked part to burn the economic value of a locked part. Inherited `b_min` and its time bound are unchanged. Normative sims and vectors must be regenerated; old run-residual figures are not new measurements. |
| L40: minimum rent exceeds collateral gap | **Dangerous in-bounds configuration confirmed; open configuration-policy choice.** | A validated Tier A configuration with `cr_open=20,000`, `cr_liq=18,000`, rate base 50,000, slopes 1,000,000 and minimum rent 86,400 seconds locks 257,534,247 lamports of minimum rent per 1 SOL notional at 89% utilization. A 2 SOL vault then falls below 18,000 bps CR at the unchanged 1 SOL quote. The focused regression validates the configuration and checks the CR. This is not evidence that current deployed/default terms are unsafe. Parent explicitly retains the relational-limit policy choice as open. |
| L41: later liquidation pays more than expiry | **Reward asymmetry confirmed; strategic delay/lender harm unresolved.** | At `V=20 SOL`, `B_ref=13 SOL` and sufficient equity, the focused waterfall check gives expiry keeper reward 0.014 SOL and liquidation reward at least 0.21 SOL. They are different eligibility paths: a healthy expired position cannot immediately use the liquidation reward. Competition, price/rent movement and keeper costs determine whether deliberate delay wins. The report's claim that delay costs nothing is not established merely by the payout formulas. No guaranteed timely-expiry mechanism or attacker profit was demonstrated; retain as open keeper-incentive policy, not an accounting fix. |
| L43: dust lender prevents last-lender vest flush | **Covered accepted vest-forfeiture behavior.** | Existing `pool_econ::exits_and_the_unreleased_vest` already tests the exact economic mechanism: a 3 SOL vest, large holder exits after 600 seconds, dust holder remains; the large holder receives 83,333,332 lamports and dust receives 2,916,666,667. Without the other holder, the last-lender flush pays the full 3 SOL. KI-09 explicitly assigns unreleased vest to the remaining shares on exit, and retaining shares until `vest_end` preserves entitlement. `LendPanel` has the vest-forfeiture acknowledgment and tests. A preexisting small lender is therefore not evidence beyond that bound. No change to forfeiture/last-lender rules. |
| L52: no borrower bound on locked minimum rent | **Execution-time exposure confirmed; open terms/API choice.** | `r_min` depends on utilization before the user's open, so an earlier open changes it without changing the user's margin/fee/proceeds bounds. The L40/L52 regression measures 13,698,631 versus 257,534,247 lamports at 0 versus 89% utilization for an in-bounds extreme configuration. This confirms the missing user bound, not a profitable front-runner strategy under defaults. Parent explicitly defers a new instruction argument or relational rent rule pending policy. |
| L74: v4 deposit recognizes pending PnL | **Distinct unresolved real-venue effect, not fixed by AIR2-09.** | `docs/research/raydium-amm.md` §2.2 describes `calc_take_pnl` assigning 12% of growth since the target-orders snapshot to `need_take_pnl`; adapter reserves subtract that non-redeemable balance. Recognizing pending protocol fees can change the economic reserves and therefore `D`, even with an exact LP ratio. The research sample measured about 0.0002% of reserves; that is a sample, not a universal bound. Existing v4 integration support explicitly emulates LP changes without this PnL change because real LP operations require target-orders state absent from the fixture. No real-ELF deposit/window-cancellation result was produced here. Required next evidence: populate that state, execute real deposit/withdraw, measure `D` and a boundary window before/after. Adding PnL back indiscriminately would count fees LPs cannot redeem and is not a justified fix. |
| L79: permissionless sync advances calendar | **Same confirmed conditional bug as L16; fixed locally.** | Permissionless `touch_pot` supplies the exact calendar advance in the regression. Calendar expiry is now irreversible even if the market's clamped accrual time is behind it. No claim is made that an attacker can manufacture a backwards network clock. |

## Additional pass-2 math keys

| Key | Disposition and evidence |
|---|---|
| `claim-sol\|hardening-inconsistent-time-base` | Same conditional calendar issue as L16/L79. The fix is the calendar watermark in `lock_of`; parent also reviews program time inputs. |
| `claim\|pot-draw-against-receivable` | **Confirmed accounting exposure; proposed patch disproved; unresolved design.** A focused public-API model credits 200 SOL rent to two old lenders, admits a later equal-total-sized depositor, realizes half the rent, and moves 50 SOL surplus into `unmatched` while 100 SOL remains receivable. An old lender's pending is 100 SOL and own realized is 25 SOL. The proposed subtraction based on **current** share of receivable is 25 SOL, so it still draws the whole 50 SOL pot; it does not repair historical accrual ownership. Claims are 75 and 25 SOL, then a withdrawal and the second loan's unpaid-rent reversal leave pending balances +21, +43 and -64 SOL. This pins a timing/accounting transfer; it does not prove profitability including complete borrower funding. KI-53 explicitly accepts late-depositor shortfall debits and the dependence on absent owners covering negative pending; those bounds explain the final deficit but do not independently establish that this first-claim race is intended or bound its cash-allocation advantage. A correction needs historical per-lender receivable/realized entitlement across deposits, exits, rolls and pot-only chains, or a different cash-realization policy. Neither an estimated current share nor globally blocking all pot draws was adopted. Parent is notified; no new state fields are silently introduced. |
| `deposit\|vested-principal-diluted-by-late-deposit` | AIR2-04 program deposit hold, with the stronger split-settlement condition below. Parent owns the marker migration and actual deposit gate. |
| `expire\|hardening-rate-unchecked` | **Documented math-domain gap fixed.** `expire` now rejects fee bps above 10,000 using the same input checker as other bps parameters. Config's compiled bound is already much lower, so no reachable program exploit was established. Test uses 10,001. |
| `liquidate\|hardening-rate-unchecked`, `liquidate\|hardening-unchecked-rate` | **Same hardening fixed** for penalty bps. Existing compiled program bounds are already at most 2,000. The test checks rejection, not an alleged deployed input bypass. |
| `min-notional-buyback-room\|buyback-room-assumes-zero-overshoot` | **Boundary guarantee confirmed; overshoot headroom remains open.** At the 53,000,000-lamport floor, Tier A minima, maximum `ert=10,000,000`, `V=106,000,000`, `K_s=11,060,000`, the first CR-liquidatable quote is 84,800,001 and max affordable quote is 84,940,000: only 139,999 lamports (about 0.165%) headroom. The focused test accepts the boundary and rejects a 0.5% overshoot. The documented derivation explicitly guarantees the boundary, not every delayed live quote; the report's stronger liveness premise is not proved by that derivation. A real first-eligible-second overshoot case remains unmeasured. Raising the floor to 67m grants chosen headroom but does not solve arbitrary gaps. Parent leaves policy unchanged. |
| `ratio\|hardening-zero-tag-full-wipe` | **Unreachable malformed-record hardening lead, not a demonstrated current bug.** A zero record's `(num,den_log1000)=(0,0)` matches the full-wipe tag, but every real record is created from `S>0` and serialized atomically; program load validates owner, discriminator, market, PDA and consecutive epoch. A zero `s_old` would cause division failure in top-up. Rejecting it earlier could strengthen the pure math boundary, but no authorized creation/read path produces it. No source change needed for the demonstrated findings. |
| `settle\|vest-flush-per-settlement-ratio` | **Confirmed bypass of the draft half-threshold; fixed.** Sequential loan removals 400,000, 240,000, 144,000, 86,400, 29,600 each remove less than half the then-current claims, yet extinguish all 900,000 borrowed tokens and leave 5 SOL vest. Every non-flushing settlement leaving live shares and nonzero vest now returns `hold=true`; no per-settlement loss threshold. Focused regression requires the hold after every part. Program uses that signal to preserve a market deposit hold until vest release; ordinary income without a settlement remains unchanged. |
| `sol-settle\|s3-wait-from-market-chain` | **Shared-history effect confirmed; open timing-policy choice.** At locked caps 300 and 500 bps, a 400-bps live fee starts the market chain for the lower cap but not the higher one. When fee crosses 500 bps after the market chain's 600-second wait, the higher-cap position is immediately S3-due if already liquidatable/expired and other amount gates pass. The focused test pins this exact predicate combination. The existing market has one chain at the minimum locked cap, not separate per-position histories. No claim of a fair-value loss is established merely by the shorter wait. Fixing this requires per-cap/per-position history or a conservative shared-reset policy; parent explicitly leaves it open. |
| `touch-pot\|hardening-chain-epoch-unchecked` | **Unreachable state-hardening lead.** A fabricated current-epoch lender with an uncleared forward chain takes the held branch and remains non-closable through third-party sync. Current owner conversion/touch paths clear that chain when becoming current. No current stored-state producer was found; a new guard is possible hardening but is not a fix for a reachable demonstrated attack. |
| `withdraw\|stale-lock-era-b-min` | **Accepted inherited-base/time bound; adversarial trigger documented.** A small first loan/withdrawal can seed `b_min` before later large loans, so later locks remain until borrowed inventory falls below that base or `lock_until`. This is the AIR-11 mechanism explicitly described in KI-13, with about 31 days default / 101 days maximum, and the existing low-base behavior is intentionally retained. The pass-2 trace clarifies that a lender can deliberately seed the era; it does not exceed that time bound. The separate ability to escape that lock with a one-token deposit **did** exceed the disclosed value bound and is fixed as L39. No blanket claim is made that every strategy using the accepted time bound is economically harmless. |
| `allowance\|allowance-cap-counts-locked-shares` (both pass-2 entries) | Confirmed and fixed as L39, including a measured control comparison beyond KI-13. |
| `withdraw\|dust-position-blocks-last-lender-flush` | Covered as L43; existing economics regression and web acknowledgment were checked. |

## Changed contracts and validation

Production math changes are deliberately limited to: raw-ratio locked depth;
ceiling/capped conversion of an existing nominal lock; calendar-watermark lock
expiry; dust-flush sweep gating and immediate source credit; a hold signal for
every settlement leaving a live vest; restart marker gating for arming; the
unlocked-value allowance cap; and out-of-domain waterfall rate rejection.
`Pool`, `Vault` and `Lender` layouts are unchanged. `SettleInputs` gains
`restart_ema_count`; program migration and marker storage are parent-owned.

The accounting regressions are in
`crates/shortwin-math/tests/ai2_accounting.rs`; the missed-restart regression is in
`settle::tests::air2_08_a_restart_rearms_even_when_no_call_observed_the_warmup`.
The focused final run passes **147 library tests and 16 accounting-target tests**
(one of the latter is the shared harness CRC self-test). The updated raw-ratio
property targets pass **38 tests**. Before/after local logs:

- `/tmp/shortwin-ai2-math-before.log`: initial conversion and dust-flush failures.
- `/tmp/shortwin-ai2-math-second.log`: calendar, split-hold and rate-domain failures.
- `/tmp/shortwin-ai2-math-accounting-final.log`: pre-L39-fix behavior, including
  `take=670224, control=336779`; all 16 evidence cases passed then.
- `/tmp/shortwin-ai2-math-focused-final.log`: all 147 + 16 focused cases after L39 fix.
- `/tmp/shortwin-ai2-math-props.log`: 38 updated property cases pass.

Commands (release **host** profile, not SBF):

```sh
CARGO_BUILD_JOBS=4 CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 cargo test --release -p shortwin-math --lib --test ai2_accounting
CARGO_BUILD_JOBS=4 CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 cargo test --release -p shortwin-math --test oracle_props --test quotes_props
```

The first full math run exposed stale whole-bps and cashback/vector expectations.
After the assigned sim owner synchronized the normative generators and fixtures,
the final full suite passed **376 tests, zero failures, zero ignored** across 27
targets (including the empty doctest target). The final generated caps fixture was
complete before the test binaries ran. Command: the same host-profile environment
with `CARGO_BUILD_JOBS=2 cargo test --release -p shortwin-math --no-fail-fast`.
Log: `/tmp/shortwin-ai2-math-full-final.log`. Changed model outputs are intentional
consequences of the documented formula changes: the risk vector has 21 truncated
buys instead of 22, still 697 exits in 1,000 runs. Historical economic figures
remain identified as historical; these are not silently replaced by passing numbers.

Strict `cargo clippy --release -p shortwin-math --all-targets -- -D warnings` passes with the same host-profile environment; `cargo fmt -p shortwin-math -- --check` and the scoped `git diff --check` pass. Log: `/tmp/shortwin-ai2-math-clippy.log`.

## Integration fixtures and execution budgets

The final lender regression rerun passes **93 tests**: `ki65_it` 19, `ki66_sec_it`
14, `ki67_sec_it` 6, `lender_it` 54. The only fixture repairs add an explicit
zero-value trader-source sweep after a dust settlement, before a new deposit or
before deliberately introducing a **new** income vest. Without that step the first
sweep now correctly represents pre-flush cash and is credited immediately under
AIR2-05. All prior economic assertions, including the exact KI-65 lamport pins,
remain unchanged. The shared lab's production-operation behavior was not changed
to silently auto-complete the gate. Log: `/tmp/shortwin-ai2-lender-rerun.log`.

On the parent-built 1.0.2 SBF artifacts below, the CI M1/M2 commands were run with
their result-writing flags and two host build jobs:

```sh
CARGO_BUILD_JOBS=2 M1_CU_WRITE=1 cargo test --manifest-path tests/bench/m1_cu/Cargo.toml --locked -- --nocapture
CARGO_BUILD_JOBS=2 M2_CU_WRITE=1 M2_TRACE_WRITE=1 cargo test --manifest-path tests/bench/m2_cu/Cargo.toml --locked -- --nocapture
```

M1 passes **62 measured cases** (two Rust tests); M2 passes **23 measured cases**
and **nine trace scenarios** (four Rust tests including trace-limit refusal checks).
Every CU measurement agrees exactly between Mollusk and LiteSVM, and every budget
and trace assertion remains unchanged. The tightest in-budget M1 case is
`cover_deficit` settling seven lagged-chain records: **59,356 / 60,000 CU**, leaving
644 CU. The existing eight-record lagged-chain rows remain explicitly outside the
web's seven-record limit and are measured against the transaction limit. Updated
files: `tests/bench/m1_cu/results.md`, `tests/bench/m2_cu/results.md` and
`tests/bench/m2_cu/trace.md`. Both standalone workspaces also pass their CI fmt and
strict Clippy commands. Final rebuilt-artifact logs: `/tmp/shortwin-ai2-m1-cu-final.log`,
`/tmp/shortwin-ai2-m2-cu-final.log`; lint logs: `/tmp/shortwin-ai2-m1-clippy.log`,
`/tmp/shortwin-ai2-m2-clippy.log`.

All **12 T-SIZE tests** pass on the final artifacts below: curve five,
PumpSwap four, Raydium three. These execute
the existing worst-case transactions through their ALTs and preserve the explicit
already-disclosed over-packet cases; no size/account limit was relaxed. The parent
fixed the independent I18 venue-view LP-ratio oracle before the final rerun.
`scripts/build-alt.sh print` succeeds (50 static-table output lines).
The final full-suite log `/tmp/shortwin-ai2-broader-all-core-final.log` confirms
these three binaries again after the last program fix. Earlier focused logs:
`/tmp/shortwin-ai2-t-size-final.log` and
`/tmp/shortwin-ai2-protocol-alt-static.txt`. T-SIZE used the same release-host
environment and `SHORTWIN_SO=.../target/deploy/shortwin.so` with:

```sh
cargo test --release --locked -p shortwin-litesvm-tests --test tx_size_raydium --test tx_size_pump_it --test tx_size_pumpswap_it -- --nocapture
```

Final measured artifact SHA-256, after the legacy/source-hold follow-up
(canonical builds owned by the parent task):

- Release `target/deploy/shortwin.so`:
  `a6ecd6f763f9549eee5913a08d57effbcd69dd312d59217a44cc386f03e62b5e`.
- Test venue `target/test-venue/deploy/shortwin.so`:
  `7449c9917d63652a7d36b9cded09a8c26137f38ce9c1513dfb7ed0d19f1505e7`.

The parent owns the final complete LiteSVM, fuzz, client and remaining CU gates.

## Normative model, fixture and final smoke evidence

The model owner synchronized `docs/design/sim/pool_model.py` with ceiling/capped
lock conversion, calendar-watermark expiry, the unlocked-share withdrawal-value
cap, dust-source sweep bits and immediate pending-source credits. Its program
integration model also checks the `deposit_hold_until` sentinel against the actual
remaining vest, including extensions and legacy zero-marker live vests.
`shortwin_sim.py` now uses the raw locked-LP depth ratio. ARCHITECTURE, ECONOMICS and
`docs/design/arch/layout.py` document these changes and the unchanged 3,104-byte
Market layout, with the new fields at 2,960 and 2,968.

Current math fixtures were regenerated for pool accounting, quote simulation,
caps, settlements and exit risk. All 20 pool fixture files match a second complete
regeneration byte-for-byte. The pool recorder explicitly disables the program-only
deposit hold and uses the trader-source requirement, matching its Rust harness;
the separate focused model tests cover both required sources. The dated attack
drivers opt into the prior arithmetic, and historical comparison tables and
`out/attacks/` figures remain historical. The simulator scope and commands are in
[`docs/design/sim/README.md`](../../design/sim/README.md).

Completed validation on the candidate before the follow-up review below:

- **12 focused Python tests** pass, including lock conversion boundaries, both
  sweep bits, vest extension, legacy live-vest migration and the unlocked-value cap.
- The pool model passes **three 60,000-operation** invariant/liveness runs;
  `out/pool_model.txt` contains the current runs followed by explicitly historical
  comparison tables.
- `shortwin_sim.py --mc 400` completes **4,800 Monte Carlo paths** and **300 exit-fuzz
  runs** (213 exits), with assertions intact; `out/full.txt` is regenerated.
- The **1,655-case** configuration battery passes with `--jobs 1`. EMA output and
  the preserved historical `pool_edge` output match their checked-in references.
- Focused Rust targets pass: **13 pool-vector**, **10 quote-simulation**, and
  **25 pool-economics** tests. The complete 376-test math result is recorded above.
- Final release fuzz smoke passes **20/20** in 58.15 seconds. Two corpus setups
  needed one real `sweep_trader` before the first post-dust deposit: CPMM carry-owed
  original action 60 and curve absent-deficit original action 57. Their original
  amounts and all economic assertions remain unchanged: carry owed is 2,149
  lamports; present lenders' absent-backed owed total is 322,230,848 lamports; the
  injected 10,000-lamport leak still triggers the bound. No invariant hook or
  `world.rs` behavior was weakened. Corpus resolution notes record the additions.
- Workspace fmt, strict workspace Clippy, strict program-only Clippy and
  `git diff --check` pass after those fixture edits.

Local evidence: `/tmp/shortwin-pool-vector-determinism.log`,
`/tmp/shortwin-config-current.txt`, `/tmp/shortwin-ai2-fuzz-smoke-final-fixed.log`,
`/tmp/shortwin-ai2-workspace-fmt-final.log`,
`/tmp/shortwin-ai2-workspace-clippy-final.log`, and
`/tmp/shortwin-ai2-program-clippy-final.log`. Commands use the same release-host
environment recorded above; the final fuzz command is
`cargo test --release --locked -p shortwin-fuzz --test smoke -- --nocapture`.

The final read-only production/model comparison found no mismatch in the raw
depth formula, unlocked-share cap, converted lock, or persisted restart-count
gate. The Python position model explicitly does not model account migrations or
settlement-arm migration; the program and dedicated Rust tests cover those paths.
The comparison identified two additional **integration gaps**, without changing
the finding dispositions above: a pre-upgrade
dust-flushed market can have zero vest and already-set sweep bits, and an empty
trader sweep followed by cashback claimed into the trader can book the reward
through an already-complete Trader source before the zero-amount VenueRewards
completion. In the simplified Python dust fixture, each ordering lets a new
depositor claim 999 of a subsequent 1,000 reward lamports. This is model/call-order
evidence, not an organic production exploit demonstration.

The parent added program-only protection in `prologue::accrue`: before release,
legacy marker 0 clears both source bits and becomes `max(now, vest_end, 1)` once;
an initialized market with surviving shares and a required pending venue-reward
bit also sets that positive marker. The later cashback may still vest through
Trader, but deposits wait for its actual release. The per-source math routing is
unchanged. The Python program-accounting helper mirrors these conditions and is
disabled by `settlement_deposit_gate=False` for pure math fixtures.

The follow-up focused run first failed the three expected legacy/source-gate
assertions, then passes **16/16** after integration. New coverage includes a
zero-vest legacy market's one-time source reset, an empty trader sweep followed by
a separate cashback claim and extended vest, ordinary income on a venue without
required rewards, and the deliberately unchanged pure-math source bookkeeping.
Logs: `/tmp/shortwin-ai2-model-source-hold-before.log` and
`/tmp/shortwin-ai2-model-source-hold-after.log`.

All **three 60,000-operation pool traces** also pass after this change. Only the
continuous 90% trace changes: 854 epoch rolls instead of 856, worst stranded
11,173 rather than 11,195 lamports, within the unchanged rounding bound. Its one
current summary row in `out/pool_model.txt` is updated; the other two current
rows, all historical tables, position simulations and pure-math fixtures are
unchanged. Log: `/tmp/shortwin-ai2-model-source-hold-fuzz.log`. The parent confirmed
both new actual PumpSwap transaction regressions on the final rebuilt SBF: legacy
source reset and split reward-claim vest hold. Both regressions fail against the
saved prior `ca057fb5…` artifact because the deposit incorrectly succeeds; see
[`docs/audit/findings.md`](../findings.md). The M1/M2 measurements above were rerun
on the final artifacts. The parent's final core/LiteSVM run also completed with
exit 0 after this program fix, including the 12 size probes above; its log is
`/tmp/shortwin-ai2-broader-all-core-final.log`.

After the follow-up rebuild, the full release fuzz smoke passes **20/20** again
in 94.44 seconds against release SHA-256
`a6ecd6f763f9549eee5913a08d57effbcd69dd312d59217a44cc386f03e62b5e` and
test-venue SHA-256
`7449c9917d63652a7d36b9cded09a8c26137f38ce9c1513dfb7ed0d19f1505e7`.
Workspace fmt, strict workspace Clippy and strict program-only Clippy also pass
on this tree. Logs: `/tmp/shortwin-ai2-fuzz-smoke-post-source-hold.log`,
`/tmp/shortwin-ai2-fmt-post-source-hold.log`,
`/tmp/shortwin-ai2-workspace-clippy-post-source-hold.log`, and
`/tmp/shortwin-ai2-program-clippy-post-source-hold.log`.

The pool recorder now explicitly passes `rewards_required=False`, matching its
Rust harness. Previously its bundled sweeps completed both bits together, so this
explicit setting does not change any trace: a fresh read-only regeneration proves
**all 20 pool JSON files byte-for-byte identical**, without rewriting files.
Log: `/tmp/shortwin-pool-vector-post-source-hold-determinism.log`.

## Browser integration on the final program artifacts

A fresh validator seeded all six positions and the production app successfully.
The focused run passes **12/12** in 12.0 minutes: `short-refresh`, all four
`lend-post-wipe` cases, both `lend-claim-pot-chain` cases, `lend-claim-sync`, and
all four `mobile-lend` cases. This uses the final test-venue artifact above;
the app build precedes the subsequent separate surviving-share cleanup action.
Command (two host build jobs):

```sh
pnpm --filter web e2e short-refresh.spec.ts lend-post-wipe.spec.ts lend-claim-pot-chain.spec.ts lend-claim-sync.spec.ts mobile-lend.spec.ts --project=chain --project=chain-phone
```

The first run identified two outdated setup steps: a newcomer deposit after the
chain's dust rebase lacked the now-required source sweep, and the phone claim
fixture opened its rent-paying position after idle observations. The setup adds
a real empty trader sweep and two real cranks across distinct validator Clock
seconds, respectively; no economic or UI assertion was weakened. The chain's
landed claim pays all **357,953** credited lamports: **342,390** own realised SOL
plus a **15,563** pot draw, leaving zero pending; the absent lender remains
unconverted. TypeScript, scoped Biome and diff checks pass.

Local logs: `/tmp/shortwin-ai2-web-e2e.log` (initial setup failures),
`/tmp/shortwin-ai2-web-e2e-final.log` (12 pass), and
`/tmp/shortwin-ai2-web-e2e-typecheck.log`. The first agent-local attempt stopped
before seeding because its sandbox denied the faucet bind; the parent ran the
normal fresh-validator command with its unrestricted tool, without changing
validator behavior or bypassing assertions.

After the separate surviving-share cleanup UI was complete, the chain scenario
was updated to review and sign that action in the browser instead of performing
its empty source sweep from Node. A second fresh-validator run on the final app
passes **both chain tests** in 8.1 minutes:

```sh
pnpm --filter web e2e lend-claim-pot-chain.spec.ts --project=chain
```

The review and landed transaction contain exactly `SweepTrader`, without a
deposit. The trader-source bit becomes complete, the epoch and share supply stay
unchanged, and the cleanup button disappears. The same deposit, five-minute rent
interval, withdrawal, close and claim assertions then pass: **348,205** credited
lamports are paid in full (**333,128** own realised SOL plus a **15,077** pot draw),
with zero pending and the absent lender unconverted. These timing-dependent
amounts are measured results, not substituted assertions. TypeScript and Biome
also pass with the final E2E spec. The app owner separately validates split reward
claims and the resulting vest hold with focused builder/component tests; the
organic mock scenario here has no venue reward claim. Final log:
`/tmp/shortwin-ai2-web-cleanup-chain-final.log`.
