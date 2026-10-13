# TODO after the AI-review fixes

The AI-review fixes (AIR-01 … AIR-24) are merged. Details, commits and tests for each item are in
[`docs/audit/ai-review/README.md`](docs/audit/ai-review/README.md); design-doc wording is in
[`docs/audit/ai-review/DOC-FOLLOWUPS.md`](docs/audit/ai-review/DOC-FOLLOWUPS.md).

The follow-ups are done on the branch `air-followups` (2026-10-12/13; not pushed, so not through
CI). Each ticked item names its commits; what is still open is under [What remains](#what-remains).

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
- [ ] Full CI run on the fixed code, new hashes in `docs/AUDIT.md` §0, a ≥ 24 h fuzz run
      (AUDIT.md §12).
  - [x] A local run of every CI job that can run here, on the integrated branch (2026-10-13,
        `621f8e1`; AUDIT.md §10, the review's README "Local gate"). `11d359b`
  - [x] The build's hashes in AUDIT.md §0, marked as not verified (local `anchor build`; no
        verifiable build, no CI). `11d359b`
  - [ ] CI on GitHub: it needs a push.
  - [ ] A ≥ 24 h fuzz run: the owner did not ask for one on 2026-10-12.

## What remains

Gathered from the follow-up tracks' reports and the integrated gate of 2026-10-13.

**Owner**
- [ ] SPEC §14 (fourteenth round) asks for two confirmations, drafted from the code and the sims,
      not from an owner decision: AIR-21 changes the B2-2 decision (an S1 now vests its take above
      market value; a genuine S1 loss still vests only its rent top-up), and AIR-14 widens §2.6
      residual 10, which O18 accepted on the `9aade3e` figures (a held drain now forces 181
      levels instead of 69; known-issues KI-23, KI-52; AUDIT.md §7).
- [ ] FE-31: approve the `lender.residuals` copy (≈ 3.34 SOL fresh, ≈ 4.54 SOL aged, from 2.06 /
      8.02); then `apps/web/src/config/disclosures.ts`, `ux.test.tsx` and FRONTEND.md change
      together.
- [ ] "No 24 h fuzz run" (2026-10-12): if it waives the release evidence rather than only this
      stage, the "not yet through CI, a ≥ 24 h fuzz run or a fix review" qualifiers change
      together: known-issues' status definition and §7 lead-in, AUDIT.md's after-the-tag status
      bullet and §6 "Fixed" bullet, and the review's README caveat.
- [ ] The indexer embeds the IDL, so it can no longer decode the two-argument `revalidate_venue`
      of a build before AIR-01 (a backfill would quarantine it). This matters only if such a
      build was ever deployed and its history must be indexed.

**Release evidence (needs CI or tools not on the local machine)**
- [ ] CI on GitHub, after a push. The jobs not run locally: `deny` (`cargo deny`, `cargo audit`;
      no lockfile changed since v1.0.0), `verifiable-build` (Docker), `web-e2e` (Playwright on a
      validator), `indexer`'s PostgreSQL and validator tests, `rpc_local` (validator).
- [ ] The verifiable build must reproduce AUDIT.md §0's AI-review hashes before they count.
- [ ] A ≥ 24 h fuzz run on the fixed program (AUDIT.md §12), and an external fix review
      (`docs/audit/findings.md`).
- [ ] The PostgreSQL indexer tests on the migrated and re-recorded inputs: `ingest_sessions`,
      `api_replayed`, `security_ingest`, and the replay harness's
      `every_session_replays_into_the_chain_state`,
      `the_assertions_hold_on_a_faithful_database_and_catch_tampering` and
      `the_analytics_reproduce_the_sessions_numbers`. Then INDEXER §13's totals (1,369
      transactions / 159 checkpoints, measured at M8; 1,499 / 171 expected) and its note.

**Figures**
- [ ] The m7-keeper-bench figures (`tests/bench/m7_keeper`, `tests/bench/m7_cascade`) still name
      v1.0.0's build. The gate of 2026-10-13 re-wrote them, but they fail `keeper_cost.py`'s
      cross-check (PumpSwap's crank 41,437 → 41,149 requested CU, its settling `sol_settle`
      74,445 → 76,992), so they were not committed: refresh them with the sim's constants,
      `docs/design/sim/out/keeper_cost.txt` and ECON §5.8 together.

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
