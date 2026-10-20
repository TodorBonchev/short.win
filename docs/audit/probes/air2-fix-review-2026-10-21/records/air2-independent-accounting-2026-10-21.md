# Independent focused accounting fix review — AIR2-01 and AIR2-P2-01

Review date: 2026-10-21. Reviewer: Codex agent `reader_fix_review`, acting separately from the author of the production accounting fixes. This is an attributed AI-assisted source and local-test review, not a review by an external audit firm.

## Source identity and independence

Reviewed production candidate: `e04a6776f1c2f5e02e581d76dd1255c416270275`. Both production fixes entered in `b815269224ba0f5d946f49de7e113220c4067041`; their conversion and allowance expressions are unchanged in the candidate. Compared their original parent formulas and the candidate control flow. Production `crates/shortwin-math/src/pool.rs` SHA-256: `3d270b521f777b314a3e86048bb5c4396bc78129d4f47eb5898942e30ad55312`.

Inputs reviewed: AI review 2 `report.md` finding 1; `pass-2-unjudged.md` L39; `triage-math-2026-10-18.md` rows AIR2-01, L39 and inherited stale-lock-era bound; `triage-pass2-2026-10-18.md`; formal `findings.md` AIR2-01/AIR2-P2-01; KI-13/KI-65; `pool::convert_steps`, `ShareEpoch::ratio`, `convert_shares`, `lock_of`, `locked_total`, `allowance`, `allowance_view`, `withdraw_in`, `deposit_in`, `resolve_in` and transactional wrappers. Also inspected the program withdrawal handler and epoch-record load/store path, and the existing accounting regression/harness.

I did not author these production math fixes or the original AIR2 accounting regressions. Earlier work in this session included frontend disclosures, VenueGuard client/readers, replay/live-venue fixtures and separate web changes. During this review I authored a **test-only strengthening** of the AIR2-01 regression after independently demonstrating a masking weakness. That addition is disclosed as reviewer-authored evidence, not a second independent author's test. No production math, program, frozen fuzz runner, fixture or service source was changed for this review.

## AIR2-01 — accept the arithmetic fix; regression weakness repaired

The old conversion floored the nominal owner lock while cumulative conversion could award that owner an additional share. The candidate uses `min(ceil(old_lock / denominator), converted_shares)`, with zero for full wipe. The validated denominator is nonzero and at most `10^36`; no unbounded product is added. A zero nominal lock stays zero. Conversion preserves the existing lock expiry and scales the calendar term downward, so it can be more conservative than aggregate locking without introducing a new expiry extension.

For a fully locked owner, cumulative converted shares are at most the ceiling of its old shares; therefore the capped ceiling lock covers every converted share. The fixture with 2,500,999 old shares and remainder 1 converts to 2,501 shares and now retains all 2,501 locked. The original report's proposed subtraction of floored unlocked shares was correctly not copied: an initially zero lock must not become a lock merely because a cumulative rounding unit is awarded.

**Independent test-strength finding:** on the unmodified candidate test, reverting only the owner lock to its old floor still passed the existing zero-allowance assertion. AIR2-P2-01's new unlocked-value cap rounds that one unlocked share's token value to zero in this fixture. This masked the AIR2-01 regression. At the parent's request I added direct stored/effective lock assertions and a zero-lock conversion control to the existing test, preserving the allowance assertion. The strengthened candidate test passes; restoring only the old floor in an isolated target-only source copy now fails with **lock 2,500 versus shares 2,501**, while the allowance fix stays enabled. This demonstrates detection of this fix's actual root cause.

**Scope/residual:** this accepts the demonstrated lock-rounding correction. I do not promote the original report's unverified larger “all free inventory” claim to an organically reproduced transaction. KI-65 conversion ordering/latent units remain separate. Ceiling versus floor can retain one additional lock unit per individual conversion before the share cap; this is a conservative share-unit difference, not a universal one-token economic bound or a proof of all conversion/lock interactions.

## AIR2-P2-01 / L39 — accept the unlocked-value cap

A dust top-up previously introduced a very small `u` while the value cap still counted all lender shares. With aggregate locks exhausting the other unlocked supply, `su = max(S - locked, u)` could equal `u`, making the free-inventory cap ineffective. The candidate caps allowance by `tokens_for_shares(pool, u)`. `withdraw_in` computes actual payout and refuses any payout above that allowance before committing lender/pool/vault/record changes. Depositing only adds its minted shares; it does not clear the previous lock.

The existing organic-operation regression is meaningful: three equal deposits, a small initial loan/withdrawal seeding `b_min`, two larger permitted loans, exhausted allowances, one repayment, then a one-token top-up. It checks the resulting 999 unlocked shares are worth zero tokens, refuses the whole-position withdrawal, and compares the subsequent loss with a no-top-up control to within one raw token. Restoring only the old whole-position value cap makes the regression fail with **allowance 670,224 instead of 0**. In the isolated old-formula source copy I also executed the actual whole-position withdrawal followed by the loss: **670,224 withdrawn, net 670,223 after the deposit, versus the control's 336,779-token residual**. This independently supports the triage's distinction from KI-13's previously disclosed +11.1% bound.

**Scope/residual:** the bound is on paid token value, not an unconditional `shares_burned <= unlocked_shares` rule. Integer-floor-equivalent or zero-token burns remain possible; they do not establish a payout beyond the unlocked-value cap. This change can reduce immediate withdrawal availability when aggregate/individual locks differ. Inherited `b_min` behavior and expiry policy remain; this review neither removes KI-13 nor re-estimates its general strategic-return bounds. No queue, payout-time guarantee, equal-return guarantee or new economic policy is inferred.

## Executed evidence

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
