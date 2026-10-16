# TODO after the AI-review fixes

The AI-review fixes (AIR-01 … AIR-24) are merged. Details, commits and tests for each item are in
[`docs/audit/ai-review/README.md`](docs/audit/ai-review/README.md); design-doc wording is in
[`docs/audit/ai-review/DOC-FOLLOWUPS.md`](docs/audit/ai-review/DOC-FOLLOWUPS.md).

The follow-ups are done on the branch `air-followups` (2026-10-12/13; the owner asked for it to
be pushed with `[skip ci]`, so it has not been through CI). Each ticked item names its commits;
what is still open is under [What remains](#what-remains).

## pump v3 and the Mobile Wallet Adapter (2026-10-13 … 16; branch `pump-v3`)

The branch `pump-v3` holds `main` (`c0e0900`), the pump catch-up of the design `pump-final`
(phases P0 … P8) and the Mobile Wallet Adapter (O-6, cherry-picked on 2026-10-15). Not pushed, not
through CI. The owner's decisions of 2026-10-13 are recorded in SPEC §14 (fifteenth round) and
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
- [ ] **G2: the owner's review** of the P8 docs and the release evidence (pump-final §15), before
      telling the auditors.
- [ ] **Tell the auditors** of the addendum (AUDIT.md §13) and of D2's scope change
      (`complete_curve` leaves the tagged `audit-1` scope) — the owner's step.
- [x] **The owner's `φ_s` direction** (accept the live-supply synthetic fee under KI-72's
      decision of 2026-10-12; KI-77): given after P1 introduced `φ_s` on 2026-10-14, confirmed by
      the owner on 2026-10-16; SPEC §14 (fifteenth round), AUDIT.md §7's pump v3 row and KI-77
      carry it.
- [ ] **The owner's real-device MWA test** before the release (FE-32; DEVELOPMENT "Mobile wallets
      on a real device"), and who hosts the HTTPS devnet and mainnet builds it needs (P1, P2).
- [ ] **Remove the two `@solana-mobile` release-age exclusions** in `pnpm-workspace.yaml` after
      2026-10-21, then `pnpm install --frozen-lockfile` again.
- [ ] **CI on GitHub** for `pump-v3` (nothing here has run in CI).
- [ ] **The verifiable build** of 1.0.1 (`scripts/verifiable-build.sh --local --compare
      target/deploy`: it needs solana-verify 0.5.2 and bubblewrap, not installed on the P8
      machine, and network for them, the image, the crates and platform-tools, which that
      session's rules did not allow (read-only Solana RPC only); or Docker mode, or CI's
      `verifiable-build` job), then the 1.0.1 hashes in AUDIT.md §0 as verified ones.
- [ ] **R-PFEE before pinning pump-fees on mainnet:** re-read `withdraw`'s accounts and authority
      on the build to be pinned (done for the 10-12 build, the stop rule did not trigger), then
      `initialize_config` or Emergency's `revalidate_venue` (PUMP-2026-10-13-5).
- [ ] **The release order:** the keeper (both TradeExtras formats) first, then the Governance
      program upgrade, then the web; at the upgrade the PumpCurve `VenueExtras` sets must be empty
      (release checklist); on a non-mainnet deployment, exit or settle cashback-curve positions
      first. **The next release deletes the keeper's V2 builder** (`pump_trade_extras_v2`).
- [ ] The Surfpool fork run of a synthetic close (pump-final §14 "Fork",
      `tests/litesvm/tests/graduation_surfpool.rs`, which still names the 2026-10-02 builds): not
      run (no Surfpool here).
- [ ] A ≥ 24 h fuzz run on 1.0.1 (AUDIT.md §12's fix policy), or the owner's waiver as for the
      AI review's batch; P7 ran 10-minute smokes and ≈ 1 h per profile.
- [ ] An external review of the adapter change (with the AI review's fixes, below).

## Decisions (owner)

The owner decided these on 2026-10-12 (the review's README, "Decisions for the owner", `63b673c`;
the register's rows in `18b65e2`).

- [x] **AIR-22**: keep the lower-median growth (liquidations no longer blocked by flat steps; the
      held sandwich gets cheaper, +2.839 vs +1.981 SOL in the sim), or revert `6c39e58`,
      `03b7762`, `8a4df78` and regenerate the vectors. **Kept** (known-issues KI-76).
- [x] **AIR-07**: keep the drained-pool deposit gate (`PoolDrained`, 6108; refuses a small share of
      honest deposits), or revert `00d3772` and regenerate the vectors. **Kept** (KI-71).
- [x] **AIR-10**: accept the burn residual (known-issues row) or add a per-second supply snapshot.
      **Accepted** (KI-72; no snapshot).
- [x] **AIR-18**: pre-extend shortwin's ProgramData to 10 MiB at deploy (~45 SOL) or accept the
      bounded `ExtendProgram` griefing (keeper retries next slot; correct KI-63). **Accepted**: no
      pre-extension; KI-63 corrected and KI-73 added (`18b65e2`, figures recomputed in
      `72103a5`); the keeper retries at its next tick (`8945087`).
- [x] **AIR-19**: accept the conservative `min_open_hard_cap_bps` mark or track caps exactly.
      **Accepted** (KI-74).
- [x] **AIR-04, AIR-11, AIR-20**: no code change; add the known-issues rows / doc wording
      (AIR-20 has an optional code fix in `docs/audit/ai-review/leads.md`). **Done**: KI-70, KI-13's
      lock-decay sentence, KI-75 (`18b65e2`); AIR-20's optional fix not taken, its residual pinned.

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
- [ ] Full CI run on the fixed code and new hashes in `docs/AUDIT.md` §0 (the ≥ 24 h fuzz run of
      AUDIT.md §12 is waived for this batch: approved by the owner 2026-10-13).
  - [x] A local run of every CI job that can run here, on the integrated branch (2026-10-13,
        `621f8e1`; AUDIT.md §10, the review's README "Local gate"). `11d359b`. The final
        review's fixes after it (`ed014ce`, `b1d86d4`, `33491a3`: the web, the WASM facade and
        CI only) ran CI's web job, the facade's tests and its smoke test (their commit messages).
  - [x] The build's hashes in AUDIT.md §0, marked as not verified (local `anchor build`; no
        verifiable build, no CI). `11d359b`
  - [ ] CI on GitHub: the owner asked for the branch to be pushed with `[skip ci]`
        (2026-10-13), so CI has not run.
  - [x] A ≥ 24 h fuzz run: **waived** for the AI-review batch, not deferred (approved by the
        owner 2026-10-13; the owner did not ask for one on 2026-10-12). No such run was made.
        `255ce60`

## What remains

Gathered from the follow-up tracks' reports and the integrated gate of 2026-10-13.

**Owner** (all approved by the owner 2026-10-13)
- [x] SPEC §14 (fourteenth round) asks the owner to confirm three changes, drafted from the code
      and the sims, not from an owner decision: AIR-21 changes the B2-2 decision (an S1 now vests
      its take above market value; a genuine S1 loss still vests only its rent top-up); AIR-09
      counts the tokens at 0 on a venue that cannot execute, so of an S3 there only `R_due` stays
      immediate (KI-24, accepted on the behaviour before it); and AIR-14 widens §2.6 residual 10,
      which O18 accepted on the `9aade3e` figures (a held drain now forces 181 levels instead of
      69; known-issues KI-23, KI-52; AUDIT.md §7). **Approved by the owner 2026-10-13:** AIR-21
      and AIR-09 confirmed, AIR-14's widening accepted; SPEC §14, known-issues, AUDIT.md §6/§7,
      the review's README (decision 9), ECON and ARCH say so. `255ce60`; the changelog rows
      (ECON #118, ARCH 79) `4b4041d`
- [x] FE-31: approve the `lender.residuals` copy (≈ 3.34 SOL fresh, ≈ 4.54 SOL aged, from 2.06 /
      8.02); then `apps/web/src/config/disclosures.ts`, `ux.test.tsx` and FRONTEND.md change
      together. **Approved by the owner 2026-10-13;** the three changed together, the figures
      checked against `sim/out/attacks/sandwich.out` (Tier A, no reserve: +3.340 / +4.543).
      `43aa127` (the decision recorded in `255ce60`)
- [x] "No 24 h fuzz run" (2026-10-12): if it waives the release evidence rather than only this
      stage, the "not yet through CI, a ≥ 24 h fuzz run or a fix review" qualifiers change
      together: known-issues' status definition and §7 lead-in, AUDIT.md's after-the-tag status
      bullet and §6 "Fixed" bullet, and the review's README caveat. **Approved by the owner
      2026-10-13: waived** for the AI-review batch (not deferred). The qualifiers now read "not
      yet through CI or an external fix review", with the waiver dated; AUDIT.md §12 and
      `findings.md` carry a dated note, their rule unchanged. `255ce60`
- [x] The indexer embeds the IDL, so it can no longer decode the two-argument `revalidate_venue`
      of a build before AIR-01 (a backfill would quarantine it). This matters only if such a
      build was ever deployed and its history must be indexed. **Approved by the owner
      2026-10-13:** accepted, no legacy decoder (INDEXER §3.6; the review's README decision 12).
      `255ce60`

**Release evidence (needs CI or tools not on the local machine)**
- [ ] CI on GitHub: the owner asked for the branch to be pushed with `[skip ci]` (2026-10-13),
      so CI has not run. The jobs not run locally: `deny` (`cargo deny`, `cargo audit`; since
      v1.0.0 the Cargo lockfiles change only workspace packages, `complete-curve` removed and
      `shortwin` 1.0.1, and `pnpm-lock.yaml` gained the Mobile Wallet Adapter's npm packages,
      which deny and audit do not cover) and `verifiable-build` (Docker). `web-e2e` and the
      indexer's PostgreSQL and validator tests ran locally on `pump-v3` (2026-10-16, AUDIT.md
      §10).
- [ ] `rpc_local` (manual, no CI job runs it): `SHORTWIN_RPC_URL=http://127.0.0.1:8899 cargo test
      --manifest-path services/Cargo.toml -p shortwin-services-common --test rpc_local -- --ignored`
      against `solana-test-validator`; last run by the keeper track at `8945087` (1 passed),
      `services/common` unchanged since.
- [ ] Release as 1.0.x (owner decision of 2026-10-02; SPEC §14): bump the version in
      `programs/*/Cargo.toml`, which `security.txt`'s `source_release` carries; then CI and the
      verifiable build on that tree, and its hashes in AUDIT.md §0 (the 1.0.0-versioned hashes
      there are a local build's, not verified). *The version is 1.0.1 since `dd24af1` (pump v3,
      above) and its local hashes are in AUDIT.md §0; CI and the verifiable build remain.*
- [ ] `scripts/mutants.sh` (CI's opt-in `mutants` job, nightly or `workflow_dispatch`): run on
      every earlier batch, not on the AI review's fixes nor on 1.0.1 (it builds into target
      directories of its own, which this machine's disk budget does not allow).
- [ ] An external fix review (`docs/audit/findings.md`).
- [x] A ≥ 24 h fuzz run on the fixed program (AUDIT.md §12): **waived** for this batch
      (approved by the owner 2026-10-13); no such run was made. `255ce60`
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
- [ ] `shortwin-gov`: a `--compute-unit-limit` option (`services/ops/src/send.rs`). It sets none,
      so a hashed revalidation after an extension (pump-fees 438,358 CU) cannot be executed with
      it today.
- [ ] Program comments "never had a pump UVA" (`engine/lender.rs`, `instructions/sweep_trader.rs`):
      `Market.pump_uva` is set on every PumpSwap market (`pump_amm::write_cache`); the clause
      means that the UVA holds no curve-era cashback.
- [ ] The attack scripts' historical and analysis rules still inherit the fixed sim; pin them.
- [ ] Keeper constants that are not config: `DEPLOY_SLOT_SLACK` (2), `NOT_DEPLOYED_ALERT_S` (30),
      `NOT_DEPLOYED_RUN_GAP_S` (5).
- [ ] Keeper limits after `a1edf78` / `bf037ef`, to document or improve: an RPC backend that
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
- [ ] An SBF build of `shortwin` with the fuzz jobs' profile (`CARGO_PROFILE_RELEASE_LTO=false`,
      16 codegen units) reported `execute_pool_switch::handler`'s frame at 4,224 B, 128 B over
      the 4,096 limit (2026-10-12, a local log). The canonical builds (LTO fat, 1 codegen unit)
      report nothing; CI builds the program with those. Check whether the frame can shrink.
