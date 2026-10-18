# Draft 1.0.3 implementation and validation

Date: 2026-10-18. Base: `b815269224ba0f5d946f49de7e113220c4067041` (draft 1.0.2).
These checks concern the subsequent unreleased fixes. They are separate from the
immutable static scan and from any tagged, deployed or externally reviewed release.

## Changes

- AI3-F01/F02: match the on-chain candidate-price floor and continue across failing
  candidates, including maintenance of an existing fast-path failure chain.
- AI3-F03: resolve PumpSwap creator templates for current, previous and stable extras.
- AI3-F04/F05: rank with the raw LP fraction and refresh the pool counter, vault and mint
  together. Each request contains at most 33 complete triples (99 keys). The unchanged
  default 200-key budget ranks 66 pools per tick; custom budgets must be at least 3.
- AI3-F06: pay the final lender each carry's whole lamports, normalize realised surplus
  into the pot, and clear both carries. Only sub-lamport residues become dust. The
  normative Python model, 15 pool replay files and six web-vector values follow this rule.
- AI3-L01/L03/L04: reject truncated ALT metadata and unsupported typed account versions;
  preserve the complete ranking batch across RPC failures.

Program package and all seven relevant lock entries are 1.0.3. No dependencies changed.
The IDL change is its version only; no account layout or instruction format changes.
The generated client and WASM were rebuilt. The scope generator now explicitly excludes
the previously added vendored PostgreSQL archive patch, which is used only by the optional
indexer test fixture; the core audit boundary is unchanged. Existing KI-69/KI-70 owner decisions remain;
every other item has a disposition in [triage.md](triage.md).

## Regression evidence

| Area | Negative control | Corrected behavior |
|---|---|---|
| Final carry | Three focused math regressions fail before the change. The preceding release ELF pays the departing lender 1,000,000 rather than 1,001,000 lamports; the next cohort takes the missing 1,000. | Seven focused math tests pass, including a 256-case property, mismatched carries, an earlier zero-share claimant, signed rent true-up, partial burns, sub-lamport dust and overflow rollback. Three `last_exit` LiteSVM tests pass on the new release ELF, including actual System donations/sweeps and pot-draw event accounting. |
| Fresh LP counter | Restoring discovery-time counter behavior scores a coherent deposited pool at zero instead of 50,000,000,000 lamports. | Fresh-counter regression covers proportional deposits, withdrawals and changed account bindings. |
| Rounded LP ratio | Restoring whole-bps rounding produces 49,998,000,000 instead of 50,000,000,000 lamports in the isolated control. | Raw-ratio tests pass, including a separate 0.5-bps lock that must retain positive depth. |
| Candidate floors | Restoring EMA-only evaluation accepts the 16-SOL candidate; separately restoring the early floor stop fails to propose the valid shallower candidate. | Combined verified-CPMM fixtures cover opening and improving windows and keeping the pre-armed chain alive. |
| Creator extras | Removing placeholder 4 yields only the no-extras fallback, losing all three governed sets. | Cached-creator PDA tests cover all sets, a changed creator, and unsupported venue kinds. |
| RPC retry | Restoring eager dequeue returns a different candidate on retry. | Production keeper scenario fails the first and second RPC groups, retries without discovery, and checks the pool-34 boundary and maximum request size. |
| ALT metadata | New test fails before the parser correction at an initialized four-byte header. | Lengths 4–55 are rejected; a complete empty header succeeds; a partial address body fails. All ten common-library tests pass. |
| Account versions | New supported/unsupported version scenarios exercise typed loaders rather than changing the generic byte decoder. | Config fails explicitly, unsupported markets are skipped before list-only processing, unsupported extras are unavailable, and both VenueGuard reads reject unknown versions. |

The batching scenario originally corrupted `Market.version` to force an on-chain crank
failure. With typed version rejection that market is skipped before batching, so the
scenario now corrupts its PDA bump while retaining a supported layout. The crank still
reaches the program and fails `ConstraintSeeds`; the keeper removes it from the batch,
sends the other two cranks together, and records the failed one. The focused scenario
passes, preserving the original batching behavior under test.

The fuzz corpus `fuzz-m10-24h-a-cpmm-carry-owed.json` keeps its seed and all 100 actions.
It previously asserted that carry remained unpaid. The corrected program leaves zero
unpaid and nine lamports in the vault, so the smoke case now requires that outcome and
keeps the general wind-down oracle's other rounding bounds. The focused release replay
and all 21 release smoke tests pass.

The old release binary used for the carry transaction control is the draft 1.0.2 ELF
with SHA-256 `a6ecd6f763f9549eee5913a08d57effbcd69dd312d59217a44cc386f03e62b5e`.
The 1.0.3 release and test-venue hashes are recorded in [AUDIT §0](../../AUDIT.md#0-commit-ids-hashes-contacts).
Keeper negative controls reintroduced each old defect in an isolated copy; they are not
a claim that all new tests compiled unchanged against the complete old tree. One optional
additional negative run hit Cargo cache aliasing after switching source paths; it supplied
no evidence. The main-workspace rebuild then passed all 98 keeper unit tests. Focused
pool-selection scenarios passed 15 tests and typed-view scenarios passed four.

The carry allocation follows the existing final-holder vest policy. It preserves earlier
allocated claims; it does not recover each former holder's historical fraction of
unallocated carry. It also does not fix the separate pot-entitlement design issue or
repeated per-lender accrual truncation.

## Combined checks

| Check | Result |
|---|---|
| Canonical release and test-venue SBF builds | Passed; SBPF v3; local hashes in [AUDIT §0](../../AUDIT.md#0-commit-ids-hashes-contacts) |
| A49 and release `security.txt` | Passed; source release `v1.0.3` |
| Generated IDL/client | Sync, generation, typecheck and tests passed |
| WASM | Web build and Node facade smoke passed |
| Web | 1,065 tests passed, one skipped; typecheck and lint passed |
| Normative pool model | Full script passed, including 180,000 pool operations, 120,000 stranding operations and differential/lazy-conversion checks |
| Fixture reproducibility | All 21 generated pool/lend files reproduce byte-for-byte |
| Complete math suite | 382 tests passed before the final added signed-true-up test; the final seven-test carry module passes |
| Strict Clippy | Root workspace, standalone program, services workspace and standalone keeper passed |
| Scope classification | Regenerated scope matches tracked files; the prior test-only vendor addition is explicitly classified |
| Program, venue-state, LiteSVM, math-WASM and Squads core tests | 1,345 passed; five ignored |
| M1 compute | All 63 instruction cases pass. The new final-share/unequal-carry case consumes 38,264 CU on Mollusk and LiteSVM, within the unchanged 100,000-CU withdrawal budget. Strict M1 Clippy passes. |
| Fuzz smoke | 21 release smoke tests passed after updating the historical CPMM positive oracle; corpus seed/actions and general bounds are unchanged |
| Services | Complete workspace: 421 passed, six ignored; the repaired batching case, pool-34 retry and typed-version checks pass |
| Verifiable build | Pinned solana-verify 0.5.2 rebuilt an isolated snapshot of the candidate program sources; release ELF `b88aa520…a0647` was byte-identical to the canonical Anchor build. A49 and `security.txt` passed on the rebuilt ELF. Final-revision CI confirmation remains pending. |
| T-POT full traces | All five release LiteSVM traces passed, including 40 stranding seeds and the 60,000-line economic fuzz trace. The updated stranding total is 15,273 lamports, maximum 444, matching the model. |
| Dependency policy | cargo-deny 0.20.2 passed for root and services; cargo-audit 0.22.2 passed for all eight lockfiles with the five existing CI ignores. |
| Fixture integrity | 340 pinned entries and archive SHA-256 checks passed; mainnet drift check at slot 458,456,265 found no pinned program or account changes. |
| Compute and size | M2/M3/M5/M6 instruction limits, depth, trace and size passed; M7 keeper cost/cascade and CU-math primitive budgets passed on the candidate ELFs. The M7 live-RPC capacity estimate remains unverified. |

Reproduction commands, from the repository root with its pinned toolchains:

```sh
anchor build --tools-version v1.57 --arch v3 --ignore-keys
scripts/build-test-venue.sh
cargo test -p shortwin-math --locked
cargo test -p shortwin -p shortwin-venue-state -p shortwin-litesvm-tests -p shortwin-math-wasm -p squads-v4-ix --locked
SHORTWIN_SO="$PWD/target/deploy/shortwin.so" SHORTWIN_TEST_VENUE_SO="$PWD/target/test-venue/deploy/shortwin.so" MOCK_CPAMM_SO="$PWD/target/deploy/mock_cpamm.so" cargo test --manifest-path services/Cargo.toml --workspace --locked
cargo test --release --locked -p shortwin-fuzz -- --nocapture
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy -p shortwin --all-targets --locked -- -D warnings
cargo clippy --manifest-path services/Cargo.toml --workspace --all-targets --locked -- -D warnings
cargo clippy --manifest-path services/Cargo.toml -p shortwin-keeper --all-targets --locked -- -D warnings
M1_CU_WRITE=1 cargo test --manifest-path tests/bench/m1_cu/Cargo.toml --locked -- --nocapture
python3 docs/design/sim/pool_model.py
python3 crates/shortwin-math/tests/vectors/pool/gen_pool.py
python3 apps/web/src/features/lend/vectors/gen_lend_vectors.py
pnpm --filter web exec biome format --write src/features/lend/vectors/lend_vectors.json
pnpm --filter @shortwin/client idl:sync
pnpm --filter @shortwin/client generate
pnpm --filter @shortwin/client check
pnpm --filter @shortwin/math-wasm build
pnpm --filter web test
pnpm --filter web typecheck
pnpm --filter web lint
```

The pool generator rewrites its fixture directory; run it before concurrent vector
compilation/replay. Historical attack-rule snapshots and outputs were preserved.

## Remaining release gates

This is an unreleased draft, not a released fix batch. The findings tracker therefore
uses **in progress**, not **fixed**. Open economic/venue issues need resolution or explicit
acceptance within their bounds. Release still needs a full gate on the final revision,
CI, the full browser gate, ≥24 h fuzz on the final program bytes, and external fix review.
The local verifiable build and compute/size checks passed on the candidate program bytes;
the final committed revision still needs CI confirmation. Earlier CI and fuzz evidence do not
cover 1.0.3; the AIR-01 … AIR-24 fuzz waiver does not extend to this batch.
The [fix-review packet](fix-review-1.0.3.md) records the questions and boundaries
for an independent reviewer; it is not a substitute for that review.
