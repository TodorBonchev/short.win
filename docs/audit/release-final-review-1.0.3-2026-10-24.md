# 1.0.3 finalization review — 2026-10-24

This review applies the owner's instruction to make the remaining release decisions without
another policy-choice round. It reviews `main` at `3aeac599435e4d637f4da1e3bd17807414793fa4`
plus the focused keeper recovery change in this finalization branch. It does not claim an
external audit, a completed whole-repository audit, unattended trading readiness or mainnet
deployment approval.

## Decision

**Do not create stable tag `v1.0.3` from this revision. Keep `v1.0.3-devnet.1` as the current
release marker, keep new devnet opens paused, and do not deploy to mainnet.** This is the
recommended decision because the exact program artifact is strongly qualified, but the
repository's stable-release policy also requires evidence that is still absent. A version tag
must not imply that an unfinished audit or unattended operator path passed.

This is an objective no-go, not a request for another generic owner preference. Reconsider the
stable tag when the open gates below have evidence on one exact revision.

## Evidence reviewed

| Gate | Result |
| --- | --- |
| Program artifact | Pass within its recorded scope. The deployed devnet ELF remains the canonical 1.0.3 artifact, SHA-256 `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc`. The only program-source change after the audited revision is a documentation comment in `add_margin.rs`; runtime inputs are unchanged. |
| Source CI | [CI 39922560548](https://github.com/TodorBonchev/short.win/actions/runs/39922560548) passed all executed jobs on source commit `636807d81e1b0d6361ef9ee5aee6aa4e3dc07ed3`; subsequent commits through `main` changed documentation only. The focused keeper recovery below is newer and therefore needs exact-revision CI before any tag. |
| Final fuzz | Pass for the frozen program/fuzzer/fixture inputs. The retained run completed 86,428 seconds with zero invariant failures or anomalies. It does not qualify later off-chain source or close the broader audit. |
| Whole-repository audit | Open. The 2026-10-22 deep checkpoint remains `in_progress`: seven `needs_validation` records, 23 hypotheses without independent disposition, eight blocked coverage units and two preserved provider-stopped tasks. No whole-codebase or external-audit claim is allowed. |
| Devnet instruction path | Pass only for the recorded bounded Pump open/close. Liquidation, expiry, SOL fallback, graduation and other venues are not established by that receipt. |
| Keeper and alerts | Open. The funded, market-scoped keeper and bounded smoke exist, but sustained active-position coverage, payer top-up operation, an independent alert-delivery drill and final running-artifact identities remain unproved. The market pause stays in place. |
| Reader and client | Pass for the dated observed devnet publication. This does not substitute for the operator and audit gates. |
| Live venue identities | Passed at the last dated check. Repeat the full read-only live check at the eventual exact tag revision because external program state can change. |

## Focused keeper recovery

The audit checkpoint identified a source-supported recovery gap in PumpSwap account readiness:
after successful preparation, a legitimately closed recipient WSOL ATA could disappear without
changing the derived key list. A later `VenueAccountMismatch` alerted but left the success cached,
so subsequent ticks skipped the read that would recreate the missing account.

The finalization branch invalidates PumpSwap readiness after a simulated or on-chain
`VenueAccountMismatch`. The next tick runs the existing `missing_pamm` check and permissionless
idempotent account creation. It does not add per-tick RPC work in the normal case, change the
on-chain program, or suppress the refusal alert.

Focused verification ran in an OS-isolated disposable source copy with no network and cached
lockfile dependencies only:

- `cargo test -p shortwin-keeper --lib --locked --offline`: 98 passed, 0 failed;
- targeted cache-invalidation regression: 1 passed;
- `cargo clippy -p shortwin-keeper --all-targets --locked --offline -- -D warnings`: passed;
- services-workspace formatting and `git diff --check`: passed.

This resolves the source recovery mechanism, not the prior audit run's formal record. The audit
record remains accurately `needs_validation` until a fresh independent review reconciles it.

## Required before reconsidering the stable tag

1. Commit the final release delta and obtain green full CI on that exact commit. If any program,
   math, fuzzer or fixture input changes, repeat the qualification required by the release policy;
   do not attribute the old 24-hour run to changed inputs.
2. Complete or explicitly close the audit checkpoint under its evidence contract. Preserve the
   provider-stopped gaps as gaps; do not relabel them as reviewed.
3. Prove active-position keeper coverage at the required cadence, payer top-up operation and an
   independent alert route; verify the running keeper, indexer destination `3`, API and 304-byte
   client identities. Keep new opens paused until this passes.
4. Repeat the fixture/audit-scope and live venue-identity checks on the exact intended tag commit,
   then write the final tag annotation with the artifact hash, CI run and residual-risk scope.

No transaction, deployment, stable tag or market unpause is performed by this review.
