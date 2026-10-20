# Independent AIR2 vesting and locked-depth fix review

2026-10-21. Reviewed source: `e04a6776f1c2f5e02e581d76dd1255c416270275`.

This is an independent AI source review and focused local execution record for AIR2-04, AIR2-05 and AIR2-09. This reviewer did not author these production fixes. Earlier work by this reviewer covered the full-digest guard, the FR01 Python deposit-guard mirror, service TLS and an indexer event regression. Those are separate scopes. The original findings and triage identified the mechanisms; current code and executable regressions determined the conclusions. No external firm or human audit is represented.

Recommendation: **addressed for the three original technical mechanisms in the reviewed scope**, with the limits below. An economic owner acknowledgment is not used as proof of a technical fix.

## AIR2-04

Original finding 4 (`docs/audit/ai-review-2/report.md:107`) describes new shares capturing the vested SOL paid for tokens extinguished by a halted-venue S3. Its proposed half-claims threshold also permits losses to be split across smaller settlements.

Current `pool::settle` sets `hold` whenever a non-flushing settlement leaves both shares and a live vest (`crates/shortwin-math/src/pool.rs:2437`), regardless of that individual settlement's loss fraction. Its sole production caller, `instructions/sol_settle.rs:323-328`, records the marker after removing the position, including the halted-venue case. `instructions/deposit.rs:130-135` first accrues/releases income, then requires actual shared `vest_bal == 0` while the hold marker is armed. Passing the initially recorded end time is insufficient. A successful post-release deposit resets the marker to -1.

Fresh markets initialize -1 (`create_market.rs:661`). Zero is the legacy encoding: `engine/prologue.rs:90` invalidates old source bits and arms the marker before release. The regression's legacy encoding is explicitly synthetic. The user states that the program has never been deployed; no migration requirement is inferred here.

Actual signed execution: `ai2_halted_s3_vest_it` passed on the release ELF below. It reaches S3 using real Raydium buys and the admin's real SetParams halt, then forks identical pre-settlement VM state into control and late-depositor branches. The late deposit is refused with PoolDrained. The settlement extinguishes 3,149,121,988 raw tokens and leaves 11,711,752,240 lamports vested. The original holder receives 11,711,842,813 lamports in both branches; the late depositor receives zero. The test also verifies entry after release, a zero-marker control, required cleanup after that control, and refusal at the original end after a later sweep extends the shared vest.

Math regressions additionally passed for smaller and larger settlements, a full-wipe flush without a hold, and five individually sub-threshold settlements leaving a combined 5 SOL vest. This covers the split-settlement bypass separately.

Conclusion: the demonstrated late-deposit capture and split-settlement bypass are prevented. This concerns settlement-protected historical income; the existing ordinary-income policy remains separate.

## AIR2-05

Original finding 5 (`report.md:180`) describes a dust flush retaining shares while cashback remains outside the pool. A new deposit could enter at the collapsed token price before old sources were booked.

`pool::settle` clears `wipe_swept` on every flush (`pool.rs:2433`), including one leaving shares. `deposit_in` requires Trader and, when `rewards_required`, VenueRewards source bits even when total_shares remains positive (`pool.rs:2050-2056`), before committing new shares.

`sweep_destination` (`pool.rs:377`) chooses Lenders for a pending source with surviving shares. `pool::sweep` (`pool.rs:2216`) selects that destination before setting the bit and immediately credits that first source; it does not re-vest it ahead of admission. Already-cleared ordinary source income still vests.

`instructions/sweep_trader.rs:248-264` binds supplied account keys to the market's cached trader/UVA keys and checks the pump UVA and PumpSwap UVA WSOL ATA for remaining claimable value. It marks VenueRewards complete only after those checks report clear (`371-383`). `engine/lender.rs:202` also requires rewards for markets retaining a pump UVA after leaving a pump venue.

Split claims require a second defense: an empty Trader sweep can set that source bit before a later cashback claim. A later claim arrives through Trader again and may vest. `engine/prologue.rs:96` arms the deposit hold while reward cleanup is pending and shares survive. The marker remains armed when the later sweep clears the reward bit, so the actual-vest deposit gate still prevents capture.

Actual math execution passed the dust-flush capture regression: both sources are required; 1 SOL of historical rewards is credited before admission; the newcomer receives zero; old holders receive approximately 51 SOL. The ordinary-income control passed separately: a later equal-sized lender may receive 0.5 SOL of an ordinary 1 SOL vest.

Actual signed execution passed both AIR2-05 cases in `ai2_venue_review_it`: organic dust flush with surviving shares and outstanding PumpSwap cashback; initial empty Trader sweep; separate real cashback claim and later sweep that re-vests income; refused late deposit until release; zero cashback paid to the newcomer after admission. The zero-marker/stale-source-bit control passed too.

Conclusion: the original dust-flush capture and split-claim variation are prevented for the checked sources. This does not establish recovery of cashback in an external noncanonical account or claim availability when upstream execution is unavailable. Those distinct reward-availability issues retain separate dispositions.

## AIR2-09

Original finding 9 (`report.md:418`) concerns premature whole-bps flooring. Counter/supply/equal-reserves 10,000/9,999/10,000 gave D=1; a proportional unlocked deposit changing them to 10,001/10,000/10,001 made the old formula return zero.

`pool_select::locked_depth` (`crates/shortwin-math/src/pool_select.rs:41`) now computes `floor((counter-supply)*isqrt(X*Y)/counter)` directly. lambda_bps remains a display/threshold quantity. Zero counter or supply above counter returns zero; zero reserves return zero. Two u64 reserves fit in the u128 product, and the raw locked numerator cannot exceed the counter, keeping D <= isqrt(X*Y).

Production callers use the shared helper on checked venue state: Raydium v4 (`venues/raydium_v4.rs:182`), CPMM (`raydium_cpmm.rs:240`) and PumpSwap (`pump_amm.rs:739-743`). The Raydium LP reader requires an exact 82-byte Tokenkeg mint; the venue readers bind mint/vault keys to the pool. Depth refresh, eligibility and adoption consume the resulting locked_depth. Unrelated asynchronous keeper ranking is not certified by this arithmetic review.

The normative model defaults to the same raw ratio (`docs/design/sim/shortwin_sim.py:239`, methods at 685 and 796). Historical attack drivers may explicitly select the former bps mode.

Actual tests passed the exact counterexample, scaled-reserve cases, maximum-u64 arithmetic, raw-ratio/conservative comparisons and invalid-counter boundaries. Two focused properties ran with at least 2,000 cases each: the BigUint raw-ratio comparison, and the reference-depth property using the standard integer square root. The latter bounds proportional deposits to one depth unit and checks a stated reserve-rounding bound on withdrawals.

Actual signed venue tests passed: real CPMM nine-times unlocked deposit, adoption and withdrawal preserve D within the regression tolerance; PumpSwap three-times deposit/withdrawal changes lambda from 10,000 to 2,500 bps, scales square-root liquidity fourfold, and reports depth ratio 1.000000 while preserving sizing/tier/reference bounds. These integration tests allow one-bp tolerance; the narrow math cases/properties address the integer-edge defect.

Conclusion: premature-bps rounding is addressed. Proportional invariance assumes coherent counter/supply/reserves and proportional LP changes. Reserve rounding, retained fees and real AMM-v4 PnL recognition are separate effects. L74 is not closed by this review; no real AMM-v4 deposit against target-orders/PnL state was executed here.

## Scope and availability

AI3-L10's acknowledged renewable deposit denial is distinct from technical capture prevention. Actual-vest isolation remains required even when donations prolong the hold. This review promises no universal reopening time or total cost/duration bound. Existing-owner claim/withdraw rules and the separately recorded economic choice remain unchanged.

No production source, lockfile, fixture or fuzzer input was edited. Focused native builds and evidence files used target/cache directories. This review used existing hash-checked SBF artifacts; it did not rebuild SBF, run full CI, finish the active fuzz gate or establish deployed state.

## Executed checks

Working directory `/root/short.win-audit-review`. Environment: `CARGO_BUILD_JOBS=2`, verified shared `CARGO_TARGET_DIR=/root/short.win/target`, `PROPTEST_CASES=2000`.

Runtime paths were passed explicitly through SHORTWIN_SO, SHORTWIN_TEST_VENUE_SO and MOCK_CPAMM_SO:

- `target/deploy/shortwin.so`: `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc`.
- `target/test-venue/deploy/shortwin.so`: `0a405deba60a521798820985cd4cec1a0e61e46ed9d0ee829fd037b010565a01`.
- `target/deploy/mock_cpamm.so`: `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9`.

PumpSwap tests use the current Oct2026QuoteFees profile. The dumped pump/PumpSwap ELF files hash respectively to `05249b60f8ddf02a53d0f8a43980fde40cce33f73526355b4ef08e292279819f` and `51a5f6a7fb5a812de7686e593ce6159f6ee59b15771a3d5d37ab661c46d64586`.

| Exact command | Result | Log |
|---|---|---|
| `cargo test -p shortwin-math --test ai2_accounting air2_05 --locked` | test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s | [log](air2-independent-vesting-depth/math-air2-05.log) |
| `cargo test -p shortwin-math --test ai2_accounting air2_09 --locked` | test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s | [log](air2-independent-vesting-depth/math-air2-09.log) |
| `cargo test -p shortwin-math --test ai2_accounting pass2_split_settlements_cannot_avoid_the_live_vest_hold --locked` | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.00s | [log](air2-independent-vesting-depth/math-split-settle.log) |
| `cargo test -p shortwin-math --lib a_settlement_marks_any_remaining_live_vest_for_a_deposit_hold --locked` | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 146 filtered out; finished in 0.00s | [log](air2-independent-vesting-depth/math-vest-hold.log) |
| `cargo test -p shortwin-math --test quotes_props locked_depth_properties --locked` | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 21 filtered out; finished in 0.04s | [log](air2-independent-vesting-depth/math-depth-properties.log) |
| `cargo test -p shortwin-math --test oracle_props the_depth_cap_bounds_deposit_inflation --locked` | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.04s | [log](air2-independent-vesting-depth/math-depth-cap.log) |
| `cargo test -p shortwin-litesvm-tests --test ai2_halted_s3_vest_it --locked -- --nocapture` | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.94s | [log](air2-independent-vesting-depth/svm-halted-s3.log) |
| `cargo test -p shortwin-litesvm-tests --test ai2_venue_review_it air2_05 --locked -- --nocapture` | test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 9.63s | [log](air2-independent-vesting-depth/svm-dust-split.log) |
| `cargo test -p shortwin-litesvm-tests --test pool_attacks_it unlocked_liquidity_moves_lambda_not_d --locked -- --nocapture` | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 16 filtered out; finished in 1.09s | [log](air2-independent-vesting-depth/svm-cpmm-lp.log) |
| `cargo test -p shortwin-litesvm-tests --test pamm_it a_deposit_and_its_withdrawal_move_neither_d_nor_the_bounds --locked -- --nocapture` | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 78 filtered out; finished in 8.22s | [log](air2-independent-vesting-depth/svm-pamm-lp.log) |

All 10 commands returned exit 0: 8 focused math tests (including 2 properties) and 5 signed SVM tests. The machine record [results.json](air2-independent-vesting-depth/results.json) includes exact commands, environment, return codes, log SHA256 values and runtime artifact hashes. Its SHA256 is `15b4707b6f6655a0d0005e6fac1475b2fe6976ec9a51318c82fd9d4a2cce1587`.
