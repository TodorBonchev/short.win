# TODO after the AI-review fixes

The AI-review fixes (AIR-01 … AIR-24) are merged. Details, commits and tests for each item are in
[`docs/audit/ai-review/README.md`](docs/audit/ai-review/README.md); design-doc wording is in
[`docs/audit/ai-review/DOC-FOLLOWUPS.md`](docs/audit/ai-review/DOC-FOLLOWUPS.md).

The follow-ups were completed on `air-followups` (2026-10-12/13) and integrated into `main`.
They were initially pushed with `[skip ci]`; current CI and local verification are recorded
below. Historical ticked items name their commits; the 2026-10-18 follow-up is committed as
the draft 1.0.2 audit-fix batch.

## AI review 3 and draft 1.0.3 (2026-10-18)

This is the current unreleased gate candidate; the 1.0.2 sections below are historical.
The two-pass rust-auditor scan reviewed `b815269`, before these changes. Its
[archive](docs/audit/ai-review-3/README.md) preserves all reports, source hashes,
coverage and dispositions; the [31-item register](docs/audit/ai-review-3/triage.md)
separates prior issues from new observations and records implementation decisions.

- [x] Archive eight findings and 23 leads, including the failed first-pass economic lens
      and narrower second-pass coverage. Confidence scores are not severity ratings.
- [x] Prepare the program package and lockfiles for **1.0.3**, an unreleased draft.
- [x] Complete and validate the keeper floor, candidate ordering, fresh LP-counter,
      creator-placeholder and RPC retry fixes; reject unsupported account versions.
- [x] Close the last-withdrawal carry leak, with conservation and new-cohort regressions,
      and synchronize the normative model, replay vectors and WASM/client artifacts.
- [x] Reject truncated address lookup table metadata; the regression fails before the
      parser change and all ten common-library tests pass afterward.
- [x] Record combined validation and local artifact hashes in the
      [findings tracker](docs/audit/findings.md#ai-review-3-draft-103-2026-10-18).
- [ ] Complete release gates on the final revision: remaining issue decisions, full CI,
      full browser gate, ≥24 h fuzz and external fix review. Local verifiable-build and
      compute/size checks passed on the same program bytes; CI must confirm the final
      revision. The earlier fuzz waiver covers AIR-01 … AIR-24 only.

KI-69's hashed-bootstrap procedure and KI-70's authority-transfer behavior retain their
existing owner decisions. No new acceptance of open fee, cashback, pot-entitlement or
economic-liveness issues is implied. No tag, deployment or publication is part of this draft.

## Working-tree follow-up (2026-10-18; historical 1.0.2)

This section supersedes the older “CI has not run” notes below. The changes are committed;
program changes remain a draft fix batch, not a released or externally reviewed patch.

- [x] GitHub CI was dispatched on `main` at `35558c2`: [run 39073841046](https://github.com/TodorBonchev/short.win/actions/runs/39073841046).
      The 1.0.1 verifiable build passed and reproduced `72c82879…1429`; its exact hash record
      and provenance are now in AUDIT §0. The services dependency job failed; this is not a
      green full gate, and this run does not cover the working-tree fixes.
- [x] Services dependency audit failure repaired: `rustls` 0.23.46 and `zip` 9.0.5.
      A small temporary `postgresql_archive` patch is required because upstream 0.21.0 still
      requires ZIP 8 (including for Linux Zonky JARs). No advisory exceptions added;
      cargo-deny, cargo-audit and fresh PostgreSQL download/extraction tests pass.
- [x] Android MWA timeouts diagnosed and fixed in the test harness: Chromium 152 leaves touch
      input blocked after the external wallet-protocol navigation. The fake-wallet hook cancels
      only that navigation; the real protocol and wallet exchange remain tested. All 14 formerly
      failing cases pass (19 passed, 2 flag-off skips); design-pass `STATUS.md` records evidence.
- [x] `shortwin-gov --compute-unit-limit` implemented (1–1,400,000), including outer Squads
      execution and budget-aware buffer sizing. 16 unit and 3 end-to-end tests pass, including
      hashed revalidation failing at the default budget and succeeding with 600,000 CU.
- [x] Public repository URL added to program `security.txt`; pump-UVA comments corrected;
      keeper RPC limitations and compiled constants documented. The v1 probe no longer claims
      a refused send landed.
- [x] AI review 2: initial comparison of all 13 findings and inventory of all 85 leads recorded
      in [`docs/audit/findings.md`](docs/audit/findings.md). Only finding 6 is closed as a
      duplicate; open leads still need adjudication, not blanket acceptance.
- [x] Existing draft fixes for findings 2–4 reviewed and regression-tested, including old-bucket
      cranks, freshness boundary/recovery, and a vest extended beyond the original hold end.
      IDL/client regenerated; web previews now follow the open and deposit gates.
- [x] Idle-market price refresh added to the short UI: separately reviewed observation and
      integration transactions, real chain-clock checks, reload recovery and an expiry guard.
      A local-chain end-to-end test confirms both transactions without opening a position.
      Earlier draft's web unit suite: 1,050 passed, 1 skipped; typecheck and lint passed.
- [x] Earlier draft-program compatibility checks: math/WASM 387 passed; LiteSVM 1,142 passed,
      5 ignored; fuzz smoke 20 passed after refreshing stale fixture observations. Economic
      assertions retained; strict Clippy passed for both Rust workspaces. Local draft hashes are in the
      findings tracker and are distinct from the verified 1.0.1 build.
- [ ] Release the program fix batch only after resolving the remaining findings and completing
      its full gate, exact-tree verifiable build, ≥24 h fuzz and review. The broader batch below
      implements the version bump and legacy vest/arm migration; it remains unreleased
      ([findings tracker](docs/audit/findings.md#ai-review-2-working-tree-batch-2026-10-18)).

## Broader audit triage (2026-10-18; draft 1.0.2)

- [x] Review all 85 pass-1 leads and inventory all 330 pass-2 blocks (74 distinct keys), with
      evidence and limits in the [triage register](docs/audit/ai-review-2/triage-pass2-2026-10-18.md)
      and the linked math, venue and services reports. This is targeted follow-up, not the
      missing original judge run, pass 3 or an independent audit.
- [x] Implement draft fixes for conversion-lock rounding, split-settlement vest capture,
      dust-flush source accounting, restart-stale settlement arms, raw-ratio locked depth,
      and the unlocked-share withdrawal cap. Preserve Market's 3,104-byte layout; initialize
      legacy vest/arm markers on first use and recheck legacy source bits. Split cashback
      claims hold deposits through any new vest; both edge-case transaction regressions fail
      on the preceding draft and pass on the corrected binary. Update the IDL/client, web,
      keeper and indexer. Update the indexer before the program to recognize sweep destination
      `3`; deploy the new keeper/UI with or after 1.0.2, since cleanup requires its migration marker.
- [x] Add a separately reviewed pool cleanup action when existing lenders still have shares.
      It preserves the entered deposit and lets cleanup complete before any resulting vest
      releases; deposit retries cannot roll that cleanup back by appending a deposit.
- [x] Add strict LP-mint/account parsing, guard-version checks, governance oracle preflight,
      and ALT verification fixes. Reproduce alternate-account cashback stranding against the
      pinned venue ELF; keep its recovery and completeness migration open.
- [x] Synchronize the normative Python models and replay vectors. Preserve historical attack
      rules explicitly rather than silently changing old measurements.
- [x] Bump the program package and workspace lock entries to **1.0.2**, a draft candidate.
      No tag, deployment or release has been made. Current local hashes are in AUDIT §0.
- [x] Validate the current candidate: **1,343 program/core/LiteSVM tests** pass (5 ignored),
      **376 math tests**, **21 fuzz smoke tests** and **1,065 web tests** (1 skipped), with
      typecheck, lint and strict root/program Clippy. The models pass 16 focused tests,
      180,000 pool operations, 4,800 Monte Carlo paths and the 1,655-case configuration
      battery. Regenerated pool fixtures reproduce byte-for-byte.
- [x] Integrate upstream through `9062dab` before commit/push, retaining the wallet/link UI
      and FUZZ-101-24H-A oracle fix. Its regression now adds the real observation/cleanup
      prerequisites required by 1.0.2, preserving the original corpus and economic assertions.
      The integrated web and 21-test fuzz suites, formatting and strict fuzz Clippy pass.
- [x] Refresh M1/M2/M3/M5/M6 compute and trace baselines without increasing budgets;
      all 12 transaction-size probes pass. The alternate SBF stack-frame fix passes all
      40 pool-selection tests on both the canonical and no-LTO/16-codegen-unit builds.
- [x] Refresh M7 keeper cost, cascade and PumpSwap size measurements, their simulator inputs
      and ECON §5.8 tables. All three gates pass; the largest measured market-day cost increase
      is 0.00104 SOL under the benchmark's assumptions.
- [x] Run the complete services workspace: **409 tests pass**, including PostgreSQL security,
      ingestion/replay, keeper scenarios and governance execution. Six are ignored in that run;
      the two M7 benchmarks pass separately, while four live-RPC cases remain outside it.
      Services/keeper Clippy, formatting and both offline ZIP extraction regressions pass.
- [x] Pass 12 selected validator/browser cases on the final program, including phone lending,
      post-wipe deposits, claim sync and idle-price refresh. After the cleanup UI follow-up,
      rerun the two-case claim chain on a fresh ledger: the separately reviewed sweep preserves
      shares/epoch, and the later claim pays in full. The full browser release gate remains open.
- [ ] Resolve the remaining findings: full-digest VenueGuard migration (AIR2-07), creator/risk
      fee treatment and exit-budget gap (AIR2-10/11), alternate cashback recovery (AIR2-12),
      keeper payout after rent exhaustion (AIR2-13), and historical pot entitlement
      (AIR2-P2-02). Other open lead dispositions remain in the findings tracker; unverified
      claims have not been accepted or declared fixed.

## AI review 2 of short.win 1.0.1 (2026-10-17; stopped after pass 1)

The owner asked for a rust-auditor scan of 1.0.1 (Claude Opus 5.5; 3 passes; the program and its
two crates, 109 files; proofs of concept for confidence ≥ 90), then stopped it after pass 1 and
asked for what it had found, for the auditors. Everything is in
[`docs/audit/ai-review-2/`](docs/audit/ai-review-2/README.md) (not a formal audit).

**Done**
- [x] Pass 1 of 3: 84 agents (12 specialties × 7 slices of the code), judged: 13 findings (4 at
      confidence 90, 8 at 75–89, 1 below 75) and 85 leads (`report.md`)
- [x] Proofs of concept for the 4 findings at confidence 90: 3 CONFIRMED (`open_short` and a stale
      `P_ema`, the PumpSwap price start, a deposit after a halted-venue S3), 1 UNVERIFIED
      (`convert_steps`: a safety classifier stopped its PoC agent before it wrote a test)

**Still open**
- [x] Broader targeted triage of all 85 leads and 330 pass-2 blocks, including keeper paths;
      finding 1's lock-rounding mismatch reproduced and fixed in the draft. Its original
      all-free-inventory magnitude remains unverified. Finding 6 is KI-70 / AIR-04, accepted.
- [ ] Complete the draft 1.0.2 program batch and its release gates (above).
- [ ] Original pass-2 judge run, pass 3 and independent keeper-core review; the targeted
      follow-up does not claim to complete those missing audit passes.

## Design polish: the logo and the web app's look (2026-10-16; on `main`)

A new brand mark and favicon set, and a nine-package polish of the web app at phone and desktop widths inside
FRONTEND.md §7's dark terminal. Rebased onto `main` at `d805c19` and pushed with `[skip ci]`; then every other
branch was removed (archived in `../short.win-archived-branches-2026-10-16.bundle`), so `main` is the only
branch. The full record is in [`docs/design/design-pass-2026-10/`](docs/design/design-pass-2026-10/):
`STATUS.md` (done, verification, what is left) and `CONVERSATION-SUMMARY.md` (how and why, how to continue).

**Done**
- [x] The brand mark (the W of "Win" drawn as a price chart), the favicon set, the app icons and the web manifest.
      `5ac926d`; the wallet adapter's `icon-192.png` re-rendered from the new favicon, `1011177`
- [x] P1 … P9 and the P11 cleanup: tokens and type tiers, the shell, the core components, tables and numbers,
      reading pages and empty / error states, markets, short, lend and overlays. `c41ba84` … `1ba6805`
- [x] FRONTEND.md records the pass as built (`cb91e3d`); every reference screenshot set re-shot (`9363b4c`)
- [x] Local gate on `main` with freshly built 1.0.1 programs: lint, typecheck, 1,026 unit tests, the responsive
      gate (96 passed) and the web E2E on desktop and phone green

**Still open**
- [x] **CI on GitHub** for the design pass: `web` and `web-e2e` passed in run 39073841046; the whole repository gate still has the services failure.
- [x] The 14 Android `mobile-mwa.spec.ts` timeouts: fixed and verified (working-tree follow-up above).

## pump v3 and the Mobile Wallet Adapter (2026-10-13 … 16; branch `pump-v3`)

The former branch `pump-v3` combined `main` (`c0e0900`), the pump catch-up of the design
`pump-final` (phases P0 … P8) and the Mobile Wallet Adapter (O-6, cherry-picked on 2026-10-15).
It is now integrated into `main`; the CI follow-up is recorded above. The owner's decisions of 2026-10-13 are recorded in SPEC §14 (fifteenth round) and
AUDIT.md §7; the addendum is AUDIT.md §13.

**Done**
- [x] P0 / P0b, the re-pin to pump's 2026-10-08/12 builds, then to pump's 2026-10-13 build (slot
      456,494,698): fixtures, `VENUE_GUARD_SLOTS_2026_10_13`, the harness, gates G0 and G0b,
      T-PFEE-CPI and R-PFEE's reading of the 10-12 pump-fees `withdraw`. `12b6599`, `0f3796f`,
      `53ec869`, `0c0ccf0`, `ad1b7e8`, `56857aa`; `fc02f01`, `fa40f42`, `3140431`, `a6b863b`,
      `a7867fa`
- [x] P1, the reader and the math (`B̂_v3`, the post-complete pool, ρ, `φ_s` on the live
      supply). `d0d2062`, `b066308`, `5481749`
- [x] P2, the program: every curve leg on v3, synthetic exits, `PumpMigrating` on the post-buy
      pool, `CurveCashbackCoin` (D1 (a)), D3, D4's strings. `48994cb`, `2851bec`
- [x] P3, budgets, T-LATCH and T-SELF-UPGRADE (gate G1). `1856c8b`, `6a4eb2d`, `6fbe0a6`,
      `2ec4437`
- [x] P4, the keeper: no straddle bundle, both TradeExtras formats for the upgrade window, duty-10
      alerts; the web's 10-key list and the keeper vectors. `e5c817c`, `0b412af`, `12ae9a4`
- [x] P5, the web on v3 (and the WASM `migratingFv`), the D2 removals, a `q > rT` close on the
      validator. `cb85ca7`, `5d46c3b`, `38c536e`, `02d52be`
- [x] P6, `complete_curve` retired (D2): Governance, tests and benches, the indexer, the
      workspace. `9f1a30f`, `68ddaae`, `df15ed7`, `2c402ff`
- [x] P7, fuzz (the v3 crowd, the `migrating` oracle; smokes and ≈ 1 h per profile, clean) and the
      sim on v3 (ECON §10.2's synthetic row). `3e3b43b`, `434dcbc`, `59844d6`
- [x] O-6, the Mobile Wallet Adapter on by default (packages 0.7.0 / 3.0.0, the drafted strings,
      the review's safeguards), its E2E and the reviewed Android screenshots. `5b37256`, `e79a11e`,
      `a4c2522`, `ab148e6`, `a130a25`, `02af5d3`, `8e6c227`, `8af0375`, `5b40961`
- [x] P8, version **1.0.1** (`programs/shortwin/Cargo.toml`; `security.txt`'s `source_release`
      `v1.0.1`) and the release evidence of a fresh local build: `shortwin.so` `72c82879…1429`
      (executable `0610d419…2dfc`, 1,635,424 B), test-venue `fd46da2c…9db7`, `mock_cpamm.so`
      unchanged, the three venue ELF hashes, `check --live` at slot 457,523,244 (AUDIT §0, §13).
      `dd24af1`
- [x] P8, the m7 keeper bench re-baselined on 1.0.1 (`M7_COST_WRITE`, `M7_CASCADE_WRITE`,
      `keeper_cost.py`'s constants, `out/keeper_cost.txt`, ECON §5.8): the curve's `liquidate`
      147,131 requested CU (−23.3 %). `1710890`
- [x] P8, the 21 pre-gate `minimumReleaseAge` exclusions removed (`pnpm install
      --frozen-lockfile` passes). `27df645`
- [x] P8, docs: ARCHITECTURE (changelog 81), ECONOMICS (#119, §10.2's synthetic row, §3.1's
      boost rule `⌊X₁·Y₁/10¹⁵⌋`, §5.8), SPEC §14; the AUDIT addendum (§13) and known issues
      (KI-77: `φ_s` and the fee gates' 10¹⁵ `φ`; PUMP-2026-10-13-1 … -6); the R-PUMP and R-VEN
      notes; the curve test docs and re-pin gates as built. `1f31273`, `d3a64ce`, `6ac7a78`,
      `0879185`
- [x] P8, the final local CI gate on `pump-v3` (2026-10-16, on `0879185`; AUDIT.md §10 "The
      pump v3 gate"): every CI job that runs on the machine is green, web-e2e (122 passed) and
      the indexer's PostgreSQL and validator tests included; the full LiteSVM suite 1,289 passed;
      the hashes equal §0's 1.0.1 table. The m2-cu figures it re-wrote are committed. `b44908b`
- [x] The final review's fixes (2026-10-16): the indexer stores no pump `TradeEvent` of a curve
      quoted in another token (`9246792`); T-PUMP-MATH-V3 (b″), the creator-fee rules on both
      synthetic parts against pump's ELF (`0b34288`); KI-77 (c)'s step pinned (`22fd1dc`); CI's
      t-pot comment (`a2b80e4`); the gate's record in AUDIT.md §10 and the 1.0.1 wording of AUDIT,
      known-issues, ARCH, FRONTEND, INDEXER, SPEC, SECURITY.md and the RFQ notes (`c48336c`)

**Still open**
- [ ] **Remove the two `@solana-mobile` release-age exclusions** in `pnpm-workspace.yaml` after
      2026-10-21, then `pnpm install --frozen-lockfile` again.
- [ ] **Green full CI on GitHub** for the integrated pump v3 code: run 39073841046 started on `35558c2`; services dependency audit failed.
- [x] **The verifiable build of 1.0.1:** CI run 39073841046 on `35558c2` reproduced both
      the file and executable hashes; AUDIT §0 now records the verified build. A new build
      is required for the draft program fixes.
- [ ] **R-PFEE on a new pump-fees build:** re-read `withdraw`'s accounts and authority on any
      pump-fees build after the 10-12 one (done for the 10-12 build; the stop rule did not
      trigger; PUMP-2026-10-13-5).
- [ ] **Delete the keeper's V2 builder** (`pump_trade_extras_v2`) in the release after 1.0.1.
- [ ] The Surfpool fork run of a synthetic close (pump-final §14 "Fork",
      `tests/litesvm/tests/graduation_surfpool.rs`, which still names the 2026-10-02 builds): not
      run (no Surfpool here).
- [x] A ≥ 24 h fuzz run on 1.0.1 (AUDIT.md §12's fix policy): **clean.** Run-102 (2026-10-17/18,
      the 1.0.1 bytes `72c82879…`, the fuzzer of `9062dabc`, `--seed 101000000`, 4 jobs, 86,420 s):
      35,016 runs, 6,775,400 actions, 0 failures, 0 anomalies. Run-101 before it found
      FUZZ-101-24H-A, the fuzzer's bug, not the program's (fixed in `9062dabc`;
      `tests/fuzz/README.md`). It covers 1.0.1, not the draft 1.0.2, which needs its own run.

## Code and tooling

- [x] Regenerate the IDL and the client (the `codama` CI job fails until then):
      `pnpm --filter @shortwin/client idl:sync && pnpm --filter @shortwin/client generate`.
      **Done** in `caa5852` (the two committed recordings with a pre-AIR-01 `revalidate_venue`
      migrated there); REPLAY's committed sessions re-recorded in `5c4488f`.
- [x] Web app (`apps/web`):
  - [x] `features/lend/sweep.ts`: the pump-UVA leg off the pump venues (AIR-05). `975f757`
  - [x] `features/lend/previews.ts`: `wipeSweepPending` tests `totalShares === 0n` (AIR-08);
        add `PoolDrained` to `MATH_ERRORS` (AIR-07). `2bda5f5`; `3f224c2`
  - [x] `config/errors.ts`: row 6108 `PoolDrained`; keep `errors.test.ts` in line. `3c0f602`;
        `2fea2c5`
  - [x] `features/lend/vectors/gen_lend_vectors.py`: add `PoolDrained` to `REFUSALS`, regenerate
        `lend_vectors.json` (pool_model changed in AIR-02/03/07/08/12). `3f224c2`
  - [x] Settlement previews: vest an S1 excess (AIR-21); tokens at 0 on a venue that cannot
        execute (AIR-09). AIR-21 was already in the WASM `previewSettlement`; AIR-09 `abce2b1`.
  - [x] From the final review (2026-10-13): `features/short/previews.ts` judges a position's risk
        view at AIR-10's exit risk cap (the tier at `P_ema` on PumpSwap), as the program and the
        keeper. `ed014ce`
  - [x] From the final review: the unpaid-rent preview carries AIR-03's `lender_credit_rem`
        (the WASM `RentBasis.lenderCredit` takes it). `b1d86d4`
  - [x] From the final review: CI's `web` job runs the WASM facade's Node smoke test
        (`crates/shortwin-math-wasm/js/smoke.cjs`, the only JS-side check of AIR-09's
        `executable`). `33491a3`
- [x] Keeper:
  - [x] `monitor.rs` `latch_action`: don't flag a pure `ExtendProgram` (the program answers
        `VenueNotUpgraded`, AIR-01). `d2bb168`; its review's fixes `a1edf78` (a stale or
        refused simulation's slot read whole) and `bf037ef` (a tick's flags simulated together).
  - [x] Exit predictions: use AIR-10's tier cap at `P_ema` on PumpSwap. `2c1fce9`
  - [x] S3-chain predictions: use AIR-19's market mark. `6a46cde` (and a position's S3 timer at
        its own locked cap)
  - [x] AIR-18 (from decision 4): retry a program not deployed in its slot at the next tick,
        without an alert. `8945087`
- [x] Indexer: a vest flush without an epoch roll emits no event, so `vest_bal` stays stale until
      the next `PriceCranked` (needs a new event field, IDL change). `cb756ca`
      (`SolSettle.vest_flushed`, a program change), `fb60d70` (CU figures), `590e724` (indexer).
- [x] Test harness: LiteSVM replay of KI-66 dust profile seed 66 stops (host `NegativePending`,
      program pays 0, 0); the profile was reseeded instead. `251d6e8`: the replay sends such a
      withdrawal as a refusal at `SolBelowMin`; seed 66 is back.
- [x] CI's fuzz-smoke (found failing on `4497ea6`): `deficit_tripwire_on_fuzz_m7_24h_3` and
      `wind_down_residue_on_fuzz_m10_24h_a` no longer reached their findings' shapes after AIR-07.
      One action of each reproducer revised: `70ac76e`, `621f8e1`.

## Docs and release evidence

- [x] Apply `docs/audit/ai-review/DOC-FOLLOWUPS.md` to ECONOMICS, ARCHITECTURE, SPEC, AUDIT and
      known-issues; recompute the moved sim figures and regenerate `docs/design/sim/out/`.
      `2c05eae`, `7618698`, `2aae952`, `18b65e2`, `34105c6`, `63b673c`; synced with the
      follow-ups in `72103a5` … `c6a55b5`, `be1c47a` and `b19655f`.
- [x] Ops runbook (AIR-01): after `initialize_config`, run `revalidate_venue` with the expected ELF
      hash for pump-fees, AMM v4, CPMM and Jupiter; request ≥ 0.6 M CU after an extension, or a
      limit sized by simulation (≈ 0.48 M estimated near the 10 MiB cap; KI-69 (e)). `f6e25be`;
      the CU ask raised from 0.5 M in `72103a5`, `eeee640`, `7a92078`.
- [ ] Full CI run on the fixed code and new hashes in `docs/AUDIT.md` §0.
  - [x] A local run of every CI job that can run here, on the integrated branch (2026-10-13,
        `621f8e1`; AUDIT.md §10, the review's README "Local gate"). `11d359b`. The final
        review's fixes after it (`ed014ce`, `b1d86d4`, `33491a3`: the web, the WASM facade and
        CI only) ran CI's web job, the facade's tests and its smoke test (their commit messages).
  - [x] The build's hashes in AUDIT.md §0, marked as not verified (local `anchor build`; no
        verifiable build, no CI). `11d359b`
  - [ ] Green full CI on GitHub: run 39073841046 covers the integrated committed code;
        services dependency audit failed. The current working-tree changes need a new run.

## What remains

Gathered from the follow-up tracks' reports and the integrated gate of 2026-10-13.

**Release evidence (needs CI or tools not on the local machine)**
- [ ] Green full CI on GitHub: run 39073841046 now supplies committed-tree evidence,
      including `deny`, verifiable-build, indexer and web E2E passes; services audit failed.
      Historical jobs not run locally: `deny` (`cargo deny`, `cargo audit`; since
      v1.0.0 the Cargo lockfiles change only workspace packages, `complete-curve` removed and
      `shortwin` 1.0.1, and `pnpm-lock.yaml` gained the Mobile Wallet Adapter's npm packages,
      which deny and audit do not cover) and `verifiable-build` (Docker). `web-e2e` and the
      indexer's PostgreSQL and validator tests ran locally on `pump-v3` (2026-10-16, AUDIT.md
      §10).
- [x] `rpc_local`: passed on 2026-10-18 against the local E2E validator at
      `http://127.0.0.1:18899` with the updated services lockfile (1 passed). Command:
      `SHORTWIN_RPC_URL=http://127.0.0.1:18899 cargo test --manifest-path services/Cargo.toml -p shortwin-services-common --test rpc_local -- --ignored`.
- [ ] Release as 1.0.x (owner decision of 2026-10-02; SPEC §14): the working-tree package and
      `security.txt` version are now 1.0.2; finish CI, the exact-tree verifiable build and the
      remaining release gates before tagging it. *1.0.1's hashes are reproduced by CI in
      AUDIT §0; the 1.0.2 hashes there are local test evidence only.*
- [ ] `scripts/mutants.sh`: the opt-in CI job **passed** on committed 1.0.1 in run
      39073841046. The working-tree program batch still
      needs its own mutation-test evidence.
- [x] The PostgreSQL indexer tests on the migrated and re-recorded inputs: `ingest_sessions`,
      `api_replayed`, `security_ingest`, and the replay harness's
      `every_session_replays_into_the_chain_state`,
      `the_assertions_hold_on_a_faithful_database_and_catch_tampering` and
      `the_analytics_reproduce_the_sessions_numbers`. Then INDEXER §13's totals (1,369
      transactions / 159 checkpoints, measured at M8; 1,499 / 171 expected) and its note.
      **Run locally on `pump-v3`** (2026-10-16, uid 1 through `unshare`; AUDIT.md §10):
      `ingest_sessions` 8, `api_replayed` 1, `security_ingest` 11, the replay harness 8 (nine
      sessions, 2,371 transactions, 178 checkpoints), again after the final review's indexer
      fix; INDEXER §13 carries the totals (`c48336c`). CI's run is still owed (above).

**Figures**
- [x] *Done in `1710890` (2026-10-16), on the 1.0.1 build:* the m7-keeper-bench figures
      (`tests/bench/m7_keeper`, `tests/bench/m7_cascade`) still name
      v1.0.0's build. The gate of 2026-10-13 re-wrote them, but they fail `keeper_cost.py`'s
      cross-check (PumpSwap's crank 41,437 → 41,149 requested CU, its settling `sol_settle`
      74,445 → 76,992), so they were not committed: refresh them with the sim's constants,
      `docs/design/sim/out/keeper_cost.txt` and ECON §5.8 together.
      **pump-final §12's re-baseline trigger is met** (`m7_keeper`: re-baseline if `liquidate`
      moves by more than 5 %, plus T-SIZE of the keeper's v3 exits). PUMP-V3 P3 measured the
      bench on the v3 build `b3130ae2…8e31` without writing it: the curve `liquidate` is
      133,755 consumed / 147,131 requested CU at 1,122 B (committed: 174,397 / 191,837 at
      1,452 B, −23.3 % requested), its `expire` 131,194 / 144,314 (171,831 / 189,015); PumpSwap's
      `liquidate` 214,447 requested (212,585) and its settling `sol_settle` 77,008 (74,445).
      Re-baseline after P4's keeper changes, in one commit with the items above:
      `M7_COST_WRITE=1` and `M7_CASCADE_WRITE=1`, the keeper's v3 exit sizes in `results.md`
      (`tx_size_pump_it` already asserts the curve composites), and the P8 addendum's figures.

**Re-measurements (fork or network data)**
- [ ] O19's real-venue held sandwich, the FITX-1 fork figures and ECON §10.1's graduation rows
      (marked as measured before the AI review's fixes); KI-02's Tier A and real-venue figures
      and KI-76's "about twice" (marked as not re-measured).
- [ ] Jupiter's ELF size and hash cost: above 2 MiB the fingerprint does not protect it against
      extensions. Its bit is in no v1.0 latch mask, so this blocks nothing before v1.1.

**Code and tooling (optional)**
- [x] `security.txt`'s `source_code` names this public repository in the draft program batch.
- [x] `shortwin-gov --compute-unit-limit`: implemented and tested; specify it on `execute`
      or `approve --execute`, since staging does not store the outer transaction's budget.
- [x] Program comments distinguish the cached pump UVA address from curve-era cashback.
- [x] Pin the attack scripts' historical and analysis rules separately from the current
      normative model (`docs/design/sim/README.md`); replay generators explicitly use the
      current model. The historical pool-edge report is reproduced unchanged.
- [ ] Optional: make keeper thresholds configurable. Their compiled values
      `DEPLOY_SLOT_SLACK` (2), `NOT_DEPLOYED_ALERT_S` (30), `NOT_DEPLOYED_RUN_GAP_S` (5)
      are now documented in `services/README.md`.
- [x] Keeper limits after `a1edf78` / `bf037ef` documented in `services/README.md`: an RPC backend that
      reports no context slot makes every `VenueNotUpgraded` stale (a whole read per deploy slot,
      `VenueFlagFailed` on upgrades); a pool that always simulates on a lagging node pages an
      upgrade but never flags it (three round trips a tick); a flag or revalidation between the
      header read and the simulation is booked once as an extension; no test runs the parallel
      simulations against a JSON-RPC node.
- [ ] The migrated INGEST fixture `raydium_fast_path_venue_review.jsonl` carries a signature that
      no longer signs its changed message (nothing verifies it; INGEST's fixtures have no
      recording procedure).
- [ ] Fuzz coverage since the drained-pool gate: late deposits after a drain are refused
      (83 of 1,689 deposits in a 10-minute run), so FUZZ-M7-24H-3-sized deficits are reachable
      only at ≤ 99.9 % utilisation; no pre-AIR baseline was compared.
- [x] Reproduce and fix the alternate SBF profile's 4,224-byte `execute_pool_switch::handler`
      frame. Candidate verification and carried-position fit now run in a separate frame;
      canonical and no-LTO/16-codegen-unit builds are clean, and all 40 pool-selection
      integration tests pass on both final draft artifacts. The alternate artifact is diagnostic,
      not release provenance.
