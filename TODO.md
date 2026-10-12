# TODO after the AI-review fixes

The AI-review fixes (AIR-01 … AIR-24) are merged. Details, commits and tests for each item are in
[`docs/audit/ai-review/README.md`](docs/audit/ai-review/README.md); design-doc wording is in
[`docs/audit/ai-review/DOC-FOLLOWUPS.md`](docs/audit/ai-review/DOC-FOLLOWUPS.md).

## Decisions (owner)

- [ ] **AIR-22**: keep the lower-median growth (liquidations no longer blocked by flat steps; the
      held sandwich gets cheaper, +2.839 vs +1.981 SOL in the sim), or revert `6c39e58`,
      `03b7762`, `8a4df78` and regenerate the vectors.
- [ ] **AIR-07**: keep the drained-pool deposit gate (`PoolDrained`, 6108; refuses a small share of
      honest deposits), or revert `00d3772` and regenerate the vectors.
- [ ] **AIR-10**: accept the burn residual (known-issues row) or add a per-second supply snapshot.
- [ ] **AIR-18**: pre-extend shortwin's ProgramData to 10 MiB at deploy (~45 SOL) or accept the
      bounded `ExtendProgram` griefing (keeper retries next slot; correct KI-63).
- [ ] **AIR-19**: accept the conservative `min_open_hard_cap_bps` mark or track caps exactly.
- [ ] **AIR-04, AIR-11, AIR-20**: no code change; add the known-issues rows / doc wording
      (AIR-20 has an optional code fix in `docs/audit/ai-review/leads.md`).

## Code and tooling

- [ ] Regenerate the IDL and the client (the `codama` CI job fails until then):
      `pnpm --filter @shortwin/client idl:sync && pnpm --filter @shortwin/client generate`.
- [ ] Web app (`apps/web`):
  - [ ] `features/lend/sweep.ts`: the pump-UVA leg off the pump venues (AIR-05).
  - [ ] `features/lend/previews.ts`: `wipeSweepPending` tests `totalShares === 0n` (AIR-08);
        add `PoolDrained` to `MATH_ERRORS` (AIR-07).
  - [ ] `config/errors.ts`: row 6108 `PoolDrained`; keep `errors.test.ts` in line.
  - [ ] `features/lend/vectors/gen_lend_vectors.py`: add `PoolDrained` to `REFUSALS`, regenerate
        `lend_vectors.json` (pool_model changed in AIR-02/03/07/08/12).
  - [ ] Settlement previews: vest an S1 excess (AIR-21); tokens at 0 on a venue that cannot
        execute (AIR-09).
- [ ] Keeper:
  - [ ] `monitor.rs` `latch_action`: don't flag a pure `ExtendProgram` (the program answers
        `VenueNotUpgraded`, AIR-01).
  - [ ] Exit predictions: use AIR-10's tier cap at `P_ema` on PumpSwap.
  - [ ] S3-chain predictions: use AIR-19's market mark.
- [ ] Indexer: a vest flush without an epoch roll emits no event, so `vest_bal` stays stale until
      the next `PriceCranked` (needs a new event field, IDL change).
- [ ] Test harness: LiteSVM replay of KI-66 dust profile seed 66 stops (host `NegativePending`,
      program pays 0, 0); the profile was reseeded instead.

## Docs and release evidence

- [ ] Apply `docs/audit/ai-review/DOC-FOLLOWUPS.md` to ECONOMICS, ARCHITECTURE, SPEC, AUDIT and
      known-issues; recompute the moved sim figures and regenerate `docs/design/sim/out/`.
- [ ] Ops runbook (AIR-01): after `initialize_config`, run `revalidate_venue` with the expected ELF
      hash for pump-fees, AMM v4, CPMM and Jupiter; request ≥ 0.5 M CU after an extension.
- [ ] Full CI run on the fixed code, new hashes in `docs/AUDIT.md` §0, a ≥ 24 h fuzz run
      (AUDIT.md §12). The fixes have so far only passed a local run of the CI gate.
