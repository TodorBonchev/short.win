# Independent keeper fix review for 1.0.3

## Identity and outcome

Reviewed question 2 of [the fix-review packet](fix-review-1.0.3.md):
**accepted for AI3-F01, F02, F03, F04 and F05**. No new actionable finding was
identified in this focused review. This is an independent code and regression
review of those corrections, not a release approval or a new full-system audit.

The baseline is `b815269224ba0f5d946f49de7e113220c4067041`; the integrated
candidate is `9b910602ed076a6d4359c496960b3ffc0e7c6af8`, with gate-evidence
descendant `b4abcaa6`. Tests ran from the separate review checkout at
`708746eb0085016c37f4d73e2aea02031bd6efc9`, which also incorporates the latest
main metadata changes. Independently checked
`git diff --name-only 9b910602..708746eb -- "*.rs"`: empty. The source conclusions
therefore apply to the candidate's Rust code as well as the merged Rust source.
The additional package metadata is not evidence of a fresh candidate-byte
verifiable build of the merged commit.

No repository AGENTS.md was found in the review checkout or its parent tree.
The reviewer read the packet, the old report, the implementation diff and
validation claims, then followed the actual keeper and program paths and ran
the tests below. The implementation author's narrative was not used as a
substitute for source inspection.

## Artifact identity

Independently ran SHA-256 on the downloaded run-39144291500 artifacts used in the
scenario command. They match the packet's full file hashes:

| Artifact | SHA-256 |
| --- | --- |
| release shortwin.so | `b88aa520c24301522b45871e73622a4d498603dc03a8f34ff8bbb557d7fa0647` |
| test-venue shortwin.so | `994ad12c468f465937af21080ec3c48678bb1f197ab143ab44f91f98ca56132b` |
| mock_cpamm.so | `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9` |

Artifact directory: `target/ci-39144291500/` in the review checkout.
This review did not independently rerun solana-verify or full CI.

## Source review and dispositions

| Finding | Decision | Independent reasoning and source |
| --- | --- | --- |
| AI3-F01, candidate floor reference | Accept | `services/keeper/src/pool_monitor.rs:360-392` verifies the candidate with the program's own verifier, computes its proposal floor through `window_price(p_ref, &a)`, and checks carried fit at `FitAt::Proposal`. This is the same reference and fit sequence enforced in `programs/shortwin/src/instructions/propose_pool.rs:302-329`. `programs/shortwin/src/engine/pool_select.rs:327-379` retains the distinction between eligibility at the EMA and proposal floor at min(EMA, candidate spot), using spot alone when no market reference exists. Execution's EMA rule remains in `instructions/execute_pool_switch.rs:142`; this correction concerns proposals. |
| AI3-F02, search and failure chain | Accept | `services/keeper/src/pool_monitor.rs:533-608` sorts all eligible alternatives, excludes the incumbent and current window candidate, and for either OpenWindow or ImproveWindow searches for the deepest candidate passing both its own floor and carried fit, rechecking the program decision for it. The remaining decision thresholds are monotone in depth (`programs/shortwin/src/engine/pool_select.rs:279-324`), so a deepest-candidate decision refusal does not hide a shallower decision success. At zero positions before arming, maintenance now tests whether any alternative qualifies and passes its own floor, including a nonzero existing failure-chain start. The actual observation path uses the saved plan between evaluations and queues a crank on the no-route cadence (`services/keeper/src/keeper.rs:342-361,3264-3274,3379-3382`). An armed chain with a valid shallower candidate proposes that candidate rather than stopping at the deeper floor failure. |
| AI3-F03, creator placeholder 4 | Accept | `services/keeper/src/venue.rs:657-750` resolves PLACEHOLDER_COIN_CREATOR only for PumpSwap, from its cached creator. The common resolver is used by current, previous and stable sets and preserves bumps and writability. This matches the program parser's cached-creator placeholder in `programs/shortwin/src/venues/mod.rs:1259-1268`. A changed live trade creator does not change the governed template rule: the parser itself intentionally uses the cached creator. Unsupported kinds still refuse that placeholder. |
| AI3-F04, raw LP fraction | Accept | `services/keeper/src/pool_monitor.rs:238-260` computes floor((counter - circulating supply) * SOL reserve / counter) directly, with no basis-point truncation. It uses checked subtraction, rejects a zero counter and retains the existing positive-score shortlist and deterministic tie rule at lines 264-269. A product of two u64 values fits in u128, so the multiplication cannot overflow its chosen representation. This removes the previous loss of valid sub-basis-point locks without weakening later full verification. |
| AI3-F05, refreshed coherent ranking snapshot | Accept | `services/keeper/src/pool_monitor.rs:228-246` includes pool/vault/mint in each rank triple, reparses the freshly fetched pool and rejects changed SOL-vault/LP-mint bindings or fresh implausibility. The counter used in the ratio is the fresh counter. `services/keeper/src/keeper.rs:3240-3261` groups at most 33 complete triples into each call, requires the returned length, and commits only after every group succeeds. The backend's 100-account split (`services/common/src/rpc.rs:49,261-289`) therefore cannot divide a triple. `services/keeper/src/config.rs:617-619` refuses budgets below three keys. A 200-key budget consumes 198 ranking keys, within its bound. |

The shared queue correction supporting F05 and AI3-L04 was also checked:
`pool_monitor.rs:706-725` copies the queue prefix and drains it only on commit.
A failed first or later RPC group cannot consume part of the pending prefix or
publish a partial replacement ranking. Older completed ranking batches may have
been recorded on earlier ticks; this is deliberate multi-tick ranking, not a
claim that a whole discovery uses one bank snapshot.

## Independently executed checks

Commands ran through WSL Ubuntu-24.04, from `/root/short.win-audit-review`.
The existing services build cache was reused, with Cargo output explicitly
showing compilation of shortwin-math, shortwin-venue-state, shortwin 1.0.3,
services-common and keeper from the review checkout. The scenario build likewise
compiled the review checkout's keeper-litesvm and program-test harness. This
avoids relying only on a preexisting executable with uncertain source provenance.

```sh
CARGO_TARGET_DIR=/root/short.win/services/target \
  cargo test --manifest-path services/Cargo.toml \
  -p shortwin-keeper --lib --locked -- --nocapture

CARGO_TARGET_DIR=/root/short.win/services/target \
SHORTWIN_SO=/root/short.win-audit-review/target/ci-39144291500/deploy/shortwin.so \
SHORTWIN_TEST_VENUE_SO=/root/short.win-audit-review/target/ci-39144291500/test-venue/deploy/shortwin.so \
MOCK_CPAMM_SO=/root/short.win-audit-review/target/ci-39144291500/deploy/mock_cpamm.so \
  cargo test --manifest-path services/Cargo.toml \
  -p shortwin-keeper-litesvm --test scenarios pool_select --locked -- --nocapture
```

Results:

- Keeper unit tests: **98 passed, zero failed**, 2.40 s test execution.
  Focused positive regressions cover verified CPMM candidates with a deeper
  16-SOL candidate and a valid shallower 40-SOL candidate, opening/improving a
  window and maintaining the existing unarmed chain; proportional LP deposits
  and withdrawals; changed vault bindings; a positive 0.5-bps locked share;
  queue retention; minimum snapshot budget; and all three creator extras sets.
- Pool-selection LiteSVM scenarios: **15 passed, zero failed**, 24.83 s test
  execution. This includes the production keeper retry scenario at
  `services/keeper-litesvm/tests/scenarios/pool_select.rs:1283-1355`, injecting
  failure in the first or later ranking RPC group, retaining the complete queue,
  retrying without rediscovery and checking complete triples across pool 34 and
  requests no larger than 99 keys. Broader cases cover halted incumbents, open
  positions, competing windows, venue review latches, carried-position failure
  and bounded junk-pool ranking.

The baseline diff independently confirms the removed EMA-only floor call,
early floor stop, unsupported placeholder arm, rounded LP share and cached LP
counter. The author's isolated negative-control executions were read but were
not rerun by this reviewer; the executed evidence above is a fresh run of the
candidate's positive and scenario regressions.

## Limits and remaining gates

Question 2 is accepted as a fix review; the findings must remain **in progress**
until the other AUDIT §12 requirements are satisfied, including the >=24-hour
fuzz requirement on the final program bytes. The scheduled fuzz job was not
part of the cited full CI run. This review makes no acceptance of the packet's
separately retained VenueGuard migration, fee treatment, cashback recovery,
keeper incentive, pot ownership, donation, tier or repeated-accrual issues.
The review did not send production transactions, change production state,
rebuild release ELFs or establish live-RPC capacity. No source fix was needed
within question 2.

