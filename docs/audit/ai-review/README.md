# AI review of short.win v1.0.0

An AI-assisted security review of short.win's on-chain core, the triage of its leads, and a
branch of proposed fixes. The review used Claude Opus 5.5 with the `rust-auditor` Claude Code
skill.

**This is not a formal audit.** The fixes on `ai-review-fixes` are proposals for the owner's
review. They passed a local run of the CI gate (below), but none has run in CI, had a 24 h fuzz
run or an external fix review, which [`../findings.md`](../findings.md) requires before a
finding counts as fixed.

## Scope and method

- **Reviewed commit: `55b3e23`** (2026-10-08). Six scans covered the 106 core files of
  [AUDIT.md §2](../../AUDIT.md): instructions; state and engine; venues; the venue-state crate;
  math core; math rest. Each scan ran 3 passes of 12 agents. The findings were merged by root
  cause. Two independent reviewers then re-checked each merged issue, one for reachability and
  one for impact ([`SUMMARY.md`](SUMMARY.md), [`verification.json`](verification.json)).
- **The audit tag.** `audit-1` / `v1.0.0` (`e4895c2`) is 8 commits after `55b3e23`. None of them
  touches `programs/`, `crates/` or a file listed in [`scope.txt`](../scope.txt):
  `git diff 55b3e23 v1.0.0 -- programs crates` is empty. Every issue therefore applies to v1.0.0
  as reported, at the same file and line.
- **The build.** A local build of `main` @ `9aade3e` (v1.0.0 plus one docs commit), with the
  pinned toolchain (Rust 1.98.1, Agave 4.1.2, Anchor 1.2.0, platform-tools v1.57, SBPF v3),
  reproduces AUDIT.md §0's hashes byte for byte: `shortwin.so` `4d19c582…`, `complete_curve.so`
  `f3a47201…`. The regression tests below fail on that code.
- **Leads.** The summary listed eight "leads worth a look" that its reviewers had not checked.
  Each was triaged by hand against the code, the Agave source and the normative sims
  ([`leads.md`](leads.md)). Seven are confirmed (AIR-18 … AIR-24) and one is refuted (L-1).
- **Not covered.** The keeper core is in scope in AUDIT.md, but it was not scanned: the skill
  targets on-chain code. Where a fix changes what the keeper must do, the follow-ups below say
  so.

**IDs.** AIR-01 … AIR-17 are the summary's I-01 … I-17. AIR-18 … AIR-24 are leads L-2 … L-8.

## Status

| ID | Issue | Severity | Status | Commits | Regression test |
|---|---|---|---|---|---|
| AIR-01 | The venue latch keys on the ProgramData deploy slot, which a permissionless `ExtendProgram` rewrites | Medium | Fixed; residuals (decision 5) | `86f4004` | `latch_raydium_it`, `latch_pump_it`: `air01_*` |
| AIR-02 | `vest_in` rounds the vest end up, so dust sweeps keep pushing `vest_end` later | Low | Fixed | `c1ce39f` | `pool::tests::dust_lumps_do_not_push_the_vest_end` |
| AIR-03 | `lender_credit` floors each accrual's rent credit and carries no remainder | Low | Fixed | `2a4e107` | `engine_it` `prologue::per_second_cranks_credit_a_small_positions_rent` |
| AIR-04 | `apply_config` installs new governance and pause keys without their signatures | Informational | No change; known-issues row (decision 7) | — | — |
| AIR-05 | `rewards_required` reads the current venue kind, so the wipe's cashback gate drops after a market leaves pump | Low | Fixed | `1e938e8` | `pump_it` `graduation::air05_a_full_wipe_after_the_market_left_pump_waits_for_the_curve_cashback` |
| AIR-06 | The `min_notional` floor admits positions with no buyback room at the liquidation boundary | Low | Fixed | `5bb8d87` | `params::tests::min_notional_floor_keeps_buyback_room_at_the_liquidation_boundary`; `it` `admin::min_notional_below_the_buyback_room_floor_is_refused` |
| AIR-07 | `maybe_roll_in` flushes the unreleased vest only on a full wipe, not on a rebase roll | Medium | Fixed; the drained-pool gate is separate (decision 2) | `e1f873a`; the drained-pool gate `00d3772`; indexer `b093545` | `pool_econ` `a_dust_deposit_before_the_last_settlement_takes_no_vest`, `a_deposit_above_a_thousandth_cannot_keep_the_vest_from_the_old_lenders`; `lender_it` `air_07_*` |
| AIR-08 | On an emptied non-wipe market, deposit-sweep-withdraw takes swept SOL owed to the protocol | Low | Fixed (keeper too) | `20009fd`; keeper `21229b6` | `pool_econ` `a_deposit_into_an_emptied_pool_waits_for_the_sweep`; `lender_it` `air_08_*`; `m5_sec_curve_it` `air_08_*` |
| AIR-09 | S2/S3 settlement credits `Q·P_ema` at once, at a frozen pre-halt price | Low | Fixed | `907994e` | `lender_real_it` `jit::air_09_a_jit_whale_around_an_s3_on_a_halted_venue_realises_nothing` |
| AIR-10 | The liquidation risk view prices at the live fee tier, which a same-transaction sale raises | Low | Fixed for sales; burns remain (decision 3) | `cece6fa`; fuzz oracle `84cc9c9` | `m7_sec_it` `pumpswap::air_10_a_same_transaction_sale_does_not_raise_the_exit_risk_fee` |
| AIR-11 | `withdraw_in` resets `b_min` only when `lock_norm == 0`, so later locks keep a stale low `b_min` | Informational | No change: intended design; doc wording (decision 7) | — | — |
| AIR-12 | A new lock era resets `b_min` but not Π, so Π compounds across eras down to 1 | Low | Fixed | `e153169` | `pool_econ` `lock_decay_restarts_with_every_lock_era` |
| AIR-13 | `new_raydium` seeds `L_slow` at the 2D anchor instead of capping it at the locked depth D | Low | Fixed | `07c457b`; tests `e34e285`, keeper scenarios `3fc5e78` | `engine::pool_select::tests::a_price_start_seeds_l_slow_at_most_the_locked_depth` |
| AIR-14 | Raydium's `ref_depth_cap` credits 2D, so a held-then-pulled deposit lifts `L_b` over the live pool | Low | Fixed | `ead059f` | `m4r_it` `air14_a_deposit_pulled_at_the_deadline_does_not_hold_a_raydium_expiry` |
| AIR-15 | The settlement box takes `L_hi` from the upper median of bucket highs, which deposit-wrapped cranks raise | Low | Fixed | `1ab0928` | `settle::tests::deposit_wrapped_highs_widen_l_hi_only_up_to_the_depth_cap`; the G-E4 LiteSVM tests (`lp_around_settle`) |
| AIR-16 | `evaluate` step 2 refreshes a lapsed arming clock instead of letting it re-arm | Low | Fixed | `6977467` | `settle::tests::step_2_does_not_revive_a_lapsed_clock`; `settle_it` `more::a_stale_bucket_keeps_the_clock_and_a_long_gap_re_arms` |
| AIR-17 | Under the post-change guard, the spot-leg floor reads bucket lows that withdraw-wrapped cranks lower | Low | Fixed | `7343a63`; tests `9323f98` | `bounds::tests::withdraw_wrapped_cranks_do_not_lower_the_guarded_spot_floor` |
| AIR-18 | L-2: `ExtendProgram` blocks a program for the rest of its slot | Low | Operational (decision 4) | — | — |
| AIR-19 | L-3: a live fee between a position's locked cap and the global hard cap leaves it with no forced exit | Low | Fixed; conservative mark (decision 6) | `7bd84e7` | `route_it` `hard_cap::air_19_a_fee_above_the_locked_cap_is_no_route_for_that_position` |
| AIR-20 | L-4: an LP withdrawal inside `sol_settle` removes the live veto on S1/S2 | Informational | Documented and pinned; no behaviour change (decision 7) | `eb02191` | `settle::tests::a_drained_live_state_leaves_the_box_corner_as_the_only_s1_veto` (pins the residual) |
| AIR-21 | L-5: S1 credits a take above market value at once, with no vest | Low | Fixed | `e05315f` | `lender_real_it` `jit::air_21_a_jit_whale_around_an_s1_takes_none_of_its_excess` |
| AIR-22 | L-6: one flat price step every few seconds zeroes the sustained growth and blocks liquidation in a fast pump | Low | Fixed; trade-off (decision 1) | `6c39e58`; tests `03b7762`, `8a4df78` | `observe::tests::one_flat_step_in_three_keeps_the_sustained_growth` |
| AIR-23 | L-7: exits read `pool_lock_min_bps` live, not the value locked at open | Low | Fixed by AIR-14; pinned | `f726bc6` | `m4r_it` `air23_a_governed_pool_lock_min_bps_moves_no_open_positions_exit_bound` |
| AIR-24 | L-8: a pending `EnableVenues` change undoes `disable_venue` | Low | Fixed | `a113886` | `it` `admin::air24_disable_venue_cancels_a_pending_enable_of_that_venue` |
| — | L-1: a borrower whose address refuses lamports blocks its own liquidation | — | Refuted ([`leads.md`](leads.md)) | — | — |

Each regression test fails on v1.0.0 and passes with the fix, except AIR-20's, which pins the
residual. Each fix commit message gives the root cause, the fix, the options weighed and the
test's figures before and after. Where a fix changes a normative sim (`docs/design/sim/`), the
same commit regenerates the vectors, so every commit passes the vectors check on its own.

**The branch.** It holds 32 commits on `main` @ `9aade3e`, in a linear history: 30 fix and test
commits, one that refreshes the CU benchmark figures, and the one that adds this folder. The
fixes were written in six independent groups and then combined. Where two fixes met, the later
commit carries the reconciliation:
- AIR-19 (`7bd84e7`) places its field after AIR-03's `lender_credit_rem`.
- AIR-14 (`ead059f`) also moves AIR-15's settlement-box cap to D, with its tests and the
  PumpSwap fuzz oracle. On Raydium it gives up V2-14's `L_slow` credit above D. That credit
  was added against FITV-2, a forced liquidation wrapped in an unlocked LP's withdrawal. The
  keeper's FITV-2 contest still liquidates nothing on this branch: the honest keeper does not,
  and all 390 of the attacker's wrapped liquidations are refused.
- AIR-08 (`20009fd`, `21229b6`) keeps AIR-05's pump-UVA markets in the sweep gate.
- The drained-pool gate (`00d3772`) updates the stranding-table pins that it and AIR-03 move
  together. It also makes the pool-vector generator's random withdrawals keep the last share
  while tokens are borrowed, as the program requires (`LastSharesBorrowed`): with deposits
  refused in drained pools, one trace reached that burn and T-POT's full replay stopped there.
- The keeper's freeze scenarios (`3fc5e78`) wait for `L_slow` after a listing, as AIR-13's
  harness commit does for the LiteSVM tests. Without the wait, seven scenarios opened smaller
  positions in a riskier tier, or none.

## Decisions for the owner

The owner resolved decisions 1–7 on 2026-10-12. Each item keeps its analysis and ends with the
resolution; item 8 lists the interface changes and needs no decision.

1. **AIR-22, sustained growth.** The fix ends the flat-step veto: a straddle crank every 3 s no
   longer blocks liquidation through a pump. The cost is that one non-rising step in three is
   now tolerated, so the sim's held-sandwich plan against a liquidatable borrower needs 2 held
   seconds instead of 3. On RA the attacker nets +2.839 SOL instead of +1.981. The borrower
   loses the same remainder (5.775 SOL), and lenders plus protocol lose less (0.049 / 1.257 SOL
   instead of 0.570 / 2.272). Keeping 3 s needs `kappa_growth_n` ≥ 5, which is outside its
   current box. To keep the old trade-off, revert `6c39e58`, `03b7762` and `8a4df78` and
   regenerate the vectors.

   **Resolved (owner, 2026-10-12): keep the lower median.** The trade-off is known-issues KI-76.

2. **AIR-07, the drained-pool deposit gate.** The base fix flushes the vest when a settlement
   leaves at most 1/1,000 of the token claims it found. That left a residual: an outsider who
   deposited more than that before the last settlement kept the vest vesting and diluted the
   old lenders (4.61 of a 5 SOL lump in the test). A separate commit closes it. `deposit` now
   refuses with `PoolDrained` (6108) while `S > 0`, `ΣB_Q > 0` and `1,000·T ≤ T + ΣB_Q`. Any
   repayment above 0.1 % of `ΣB_Q` reopens deposits. The cost falls on honest depositors: in
   the sims 26 of 6,653 deposits in the ECON §9.6 fuzz (90 % cap), 102 of 5,910 in the
   lock-heavy mix and 378 of 26,749 in the stranding table are refused, and while a pool is
   drained no newcomer can supply exit liquidity. To accept the residual instead, revert
   `00d3772` and regenerate the vectors.

   **Resolved (owner, 2026-10-12): keep the gate** (`PoolDrained` = 6108). Its cost to honest
   depositors is known-issues KI-71.

3. **AIR-10, burns.** A holder's burn earlier in the same transaction still raises the
   PumpSwap tier, because the fee at `P_ema` is resolved on the live mint supply. A test pins
   this. Closing it needs a per-second supply snapshot in `Market` (reserved bytes). Accept it
   with a known-issues row, or add the snapshot.

   **Resolved (owner, 2026-10-12): accept the burn residual** with known-issues KI-72; no
   snapshot.

4. **AIR-18, `ExtendProgram` griefing.** No program change can stop third-party extensions.
   Anyone can extend `shortwin` (or `complete_curve`) once per slot and block it for the rest
   of that slot, at ≈ 0.052 SOL of rent per slot at mainnet's 5,080 lamports/byte. The griefing
   ends for good once the program reaches the 10 MiB cap, after ≈ 870 slots (≈ 45 SOL, paid by
   the attacker). The options are:
   - extend `shortwin`'s ProgramData to 10 MiB at deployment (≈ 45 SOL of extra rent);
   - or accept the bounded griefing, have the keeper retry in the next slot, and correct
     KI-63 ("harmless").

   **Resolved (owner, 2026-10-12): accept the bounded griefing.** No pre-extension to 10 MiB.
   The keeper ticks once a slot by default, so it sends a blocked exit again at its next tick,
   in a later slot. KI-63 is corrected, the bounds are known-issues KI-73, and
   `services/README.md` "Governance" says what operators see.

5. **AIR-01, operations.** After `initialize_config`, run one `revalidate_venue` with the
   expected ELF hash for each venue whose ProgramData can still grow (pump-fees, AMM v4, CPMM,
   Jupiter). This records the fingerprints. Until then, one extension can still latch a venue.
   `shortwin-gov stage revalidate-venue` (package `shortwin-ops`) prints and stages the hash. A
   flag or revalidation after an extension hashes the trimmed ELF: request ≥ 0.5 M CU (≤ 0.44 M
   measured).

   **Resolved (owner, 2026-10-12): the runbook step**, in `services/README.md` "Governance".
   `shortwin-gov` sets no compute-unit limit, so it gets the runtime's default: 200,000 CU for
   a bare `execute`, 400,000 for `approve --execute` (two Squads instructions). Neither covers
   pump-fees' hash path, so an execution after an extension goes through a client that
   requests ≥ 0.5 M CU (Jupiter's cost is not measured: sized by simulation). A new staging at
   the current deploy slot also works, unless another extension lands before it executes. The
   residuals are known-issues KI-69 and are accepted.

6. **AIR-19, the market mark.** `min_open_hard_cap_bps` stays at the lowest cap any position
   locked since the market last emptied. This is conservative: a closed position may have set
   it. Accept it, or track each cap exactly, which needs more state.

   **Resolved (owner, 2026-10-12): accept the conservative mark** (known-issues KI-74).

7. **No behaviour change: AIR-04, AIR-11, AIR-20.** [`DOC-FOLLOWUPS.md`](DOC-FOLLOWUPS.md) has
   the known-issues rows and doc wording for each. AIR-20 also has an optional code fix in
   [`leads.md`](leads.md).

   **Resolved (owner, 2026-10-12): no behaviour change.** Known-issues KI-70 (AIR-04), the
   lock-decay sentence of KI-13 (AIR-11) and KI-75 (AIR-20); AIR-20's optional fix is not
   taken, and its residual stays pinned.

8. **Interface changes.** The account sizes are unchanged; the new fields take reserved bytes.
   - `Market.lender_credit_rem` (AIR-03, @2,936) and `Market.min_open_hard_cap_bps` (AIR-19,
     @2,952).
   - `VenueGuard.guard_len` / `guard_elf` (AIR-01).
   - `revalidate_venue(venue, expected_slot, expected_elf_hash)` (AIR-01).
   - `min_notional` has a compiled floor of 53,000,000 lamports (AIR-06), so Governance can no
     longer set it below 0.053 SOL.
   - A new error, `PoolDrained` = 6108 (AIR-07); the existing codes keep their numbers.
   - `EpochRolled.vest_flushed` is also reported on a rebase that flushed (AIR-07).
   - `sweep_trader` takes the trader's pump UVA as a one-account token leg on a market that
     left pump (AIR-05).
   - `disable_venue` can now emit `ConfigCancelled` (AIR-24).

## Follow-ups not done on this branch

- **IDL and `clients/js`.** These are not regenerated, so CI's `codama` job fails until
  someone runs `pnpm --filter @shortwin/client idl:sync && pnpm --filter @shortwin/client generate`
  on a build of this branch.
- **Web app.**
  - `apps/web/src/features/lend/sweep.ts` must add the pump-UVA leg off the pump venues
    (AIR-05).
  - `previews.ts`' `wipeSweepPending` must test `totalShares === 0n` instead of
    `wipeLive(market)` (AIR-08).
  - `PoolDrained` (6108) needs a row in `apps/web/src/config/errors.ts` and in `previews.ts`'
    `MATH_ERRORS`, and a `REFUSALS` entry in `gen_lend_vectors.py` (AIR-07).
  - `lend_vectors.json` must be regenerated: `pool_model` changed in AIR-02, AIR-03, AIR-07,
    AIR-08 and AIR-12.
  - The settlement previews must vest an S1 excess (AIR-21) and price the tokens at 0 on a
    venue that cannot execute (AIR-09).
- **Indexer.** `vest_bal` is zeroed at every flush that rolls the epoch (AIR-07). A flush
  without a roll emits no event, so `vest_bal` stays stale until the next `PriceCranked`.
  Closing that needs a new event field, which changes the IDL.
- **Test harness.** In one KI-66 dust profile (seed 66), the LiteSVM replay of the pool trace
  stops at one withdrawal: the host model refuses it with `NegativePending` and the program pays
  it out (0, 0). The math crate and the model agree on every vector. The profile was reseeded,
  and the replay needs a follow-up.
- **Keeper.** The keeper core builds and its tests pass on this branch (see the gate). These
  behaviours still need a decision or a change:
  - `monitor.rs`' `latch_action` flags any deploy-slot change, but after a pure extension the
    program now answers `VenueNotUpgraded` (AIR-01).
  - The exit predictions should use AIR-10's tier cap on PumpSwap. Without it the keeper may
    send liquidations the program refuses, which wastes fees but is not unsafe.
  - The S3-chain predictions should use AIR-19's market mark.

  The keeper changes the fixes needed are on this branch: AIR-05 (the UVA leg and the wipe
  gate) and AIR-08 (`wipe_pending` tests `S = 0`).
- **Design docs and known issues.** [`DOC-FOLLOWUPS.md`](DOC-FOLLOWUPS.md) has the wording.
- **Release evidence.** A new batch of program fixes needs the rest of AUDIT.md §12's steps: a
  full CI run, the new hashes in AUDIT.md §0 and a ≥ 24 h fuzz run. The CU figures are already
  refreshed (`a8f9994`).

## Local gate

CI was not run on this branch. The CI jobs ran locally instead, in WSL (Ubuntu 24.04) with the
pinned toolchain, on this branch's build:

| CI job or step | Result |
|---|---|
| `fmt`; `clippy -D warnings` (the workspace, `shortwin-math` and `shortwin` alone) | pass |
| `shortwin-math`, `shortwin-math-wasm`, `squads-v4-ix` tests | 374 passed |
| Vectors: every `gen_*.py` regenerated, no diff | pass |
| `config_check.py` prints OK; the config-gate tests | pass |
| The WASM build; the T-LAYOUT wasm32 check | pass |
| Audit scope; fixture checksums and `dump-fixtures.py check` | pass |
| Release and test-venue SBF builds: SBPF v3 flags, A49, security.txt | pass |
| `shortwin` unit tests | 75 passed |
| LiteSVM suite (`shortwin`, `shortwin-venue-state`, `shortwin-litesvm-tests`) | 1,250 passed, 4 ignored ¹ |
| T-POT: every pool trace replayed in full | pass (stranding table: 15,044 lamports, worst seed 435) |
| Services: fmt, clippy, the keeper and `shortwin-gov` tests, the 121 keeper LiteSVM scenarios, the keeper binary's A49 check | 226 passed, 3 ignored |
| CU benches (math, M1–M6) | every budget holds; figures refreshed in `a8f9994` |

¹ The four ignored tests are ignored on `main` too: they need a Surfpool fork or the pre-M4R
builds. The suite ran before the last two test-only changes. `lender_it`, the only binary that
reads the regenerated pool trace, was rerun on the final tree (54 passed).

Not run here:
- `cargo deny` and `cargo audit` (neither lockfile changed);
- the `indexer` job's PostgreSQL tests (the indexer's unit tests pass: 63);
- the `codama`, `web` and `web-e2e` jobs (the IDL and the clients are not regenerated);
- the verifiable build (Docker);
- a fuzz run.

The build's hashes: `shortwin.so` `eafc9833…`; `complete_curve.so` `f3a47201…` (unchanged from
v1.0.0).

## Files

| File | Contents |
|---|---|
| [`SUMMARY.md`](SUMMARY.md) | The 17 merged issues, each with both reviewers' verdicts, the corrected impact and a fix direction |
| [`report-1-instructions.md`](report-1-instructions.md) … [`report-6-math-rest.md`](report-6-math-rest.md) | The six scan reports, unedited |
| [`verification.json`](verification.json) | Both reviewers' full reasoning for every issue |
| [`leads.md`](leads.md) | The triage of the eight leads (L-1 … L-8) |
| [`DOC-FOLLOWUPS.md`](DOC-FOLLOWUPS.md) | The design-doc and known-issues changes each fix implies, with line numbers against `9aade3e` |
