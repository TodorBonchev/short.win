# short.win audit package

This is the auditors' entry point to short.win's on-chain code. It says what is in scope, how
to build and test the code, what the code relies on, and what we already know about it.

**Status:**
- This is the M10 final package (WP-10 final), written on the merged M10 tree (`m1-core`,
  §0). Every M10 work package has landed: program code is final for the audit (batch 4; the
  release ELF `bbef1075…` since).
- The final tree's ≥ 24 h fuzz run is clean and CI is green on the program-final head
  `4f4849e` (§10).
- What is still pending is named where it applies: the owner's review of this package and the
  tag (§0), CI on the head that carries this file (§10; no program change since `4f4849e`),
  the open owner items (§7), the calibration results (KI-14) and the external audit itself
  (§12).
- Normative sources:
  - the design documents `docs/SPEC.md`, `docs/design/ARCHITECTURE.md` (ARCH) and
    `docs/design/ECONOMICS.md` (ECON);
  - this file;
  - `docs/audit/`.
- Section numbers like "ARCH §8.3" refer to those documents.

| File | What it holds |
|---|---|
| `docs/AUDIT.md` | this guide |
| [`docs/audit/scope.txt`](audit/scope.txt) | the in-scope files, generated from `git ls-files` (`scripts/audit_scope.py`) |
| [`docs/audit/known-issues.md`](audit/known-issues.md) | the known-issues register (KI-01 …): accepted and disclosed residuals, each with its bound |
| [`docs/audit/findings.md`](audit/findings.md) | the findings tracker (the auditors' findings and their resolution) |
| [`docs/audit/README.md`](audit/README.md) | the folder index and the RFQ log |
| [`SECURITY.md`](../SECURITY.md) | the vulnerability disclosure policy |

---

## 0. Commit, ids, hashes, contacts

| Item | Value |
|---|---|
| Audit commit / tag | the head of `m1-core` that carries this file: the M10 tree (`98b1181`, batch 3 from `391aed3`, the doc sync and this package; program batch 4 — the program change `1e4a273`, `3b33ab7` bytecode-neutral — with its docs and gate up to `4f4849e`), then the final tree's fuzz fixes `997c730` and `945d45a` (`tests/fuzz` and one test-harness doc comment only) and this evidence sync; the program bytes are those of `4f4849e`. **To be tagged `audit-1` / `v1.0.0` by the owner** (owner decision of 2026-10-02: release 1.0.0 at the audit tag; post-audit fixes are 1.0.x). The tag goes on the commit whose hashes match the table below |
| Package version | `shortwin` and `complete_curve` are both 1.0.0 (`programs/*/Cargo.toml`); `security.txt`'s `source_release` reads `v1.0.0` |
| `shortwin` program id | `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK` (`programs/shortwin/src/lib.rs` `declare_id!`) |
| `complete_curve` program id | `4B5mGykX9EWwaYdbTvsp8qXCUTbc7caQourz16Gat6LJ` (`programs/complete_curve/src/lib.rs`) |
| TEST-ONLY mock venue | `BPAZFPkmVtYEWEZHCzFUns7oybh1VWfAvmu3ofM2mQi7` (`tests/mock_cpamm`; never deployed; out of scope, §2) |
| Toolchain | Anchor CLI and `anchor-lang` 1.2.0; Agave CLI 4.1.2; platform-tools v1.57 (rustc 1.95), SBPF v3; host Rust 1.98.1 (`docs/DEVELOPMENT.md` "Pins") |
| Build | `anchor build --tools-version v1.57 --arch v3 --ignore-keys` (never a bare `cargo build-sbf`; never `anchor keys sync`, which would rewrite `declare_id!` and so the hashes below) |
| Verifiable build | solana-verify 0.5.2, image `solanafoundation/solana-verifiable-build@sha256:2e0b78f44ee76612e9260c7c988570c5e14de6fbd93e0ab07115ec7054473b4f`, `--arch v3 --cargo-build-sbf-args="--tools-version v1.57"`: `scripts/verifiable-build.sh` (Docker, or `--local`: a bubblewrap shim running the same digest-pinned image) and CI's `verifiable-build` job, which fail unless each output is byte-identical to the `anchor build` artifact (ARCH §1.1) |
| Contacts | Security reports: `security@short.win`; policy page `https://short.win/security` (`SECURITY.md`; the same values are in both programs' `security.txt` and the web's `/.well-known/security.txt`; owner decision Q3 of 2026-10-02). The auditors' point of contact is named at the engagement (§12) |

**Verified hashes of this tree.** `scripts/verifiable-build.sh --local --compare target/deploy`
on `m10/batch4` (2026-10-04, commit `f08426d`; its `verifiable-hashes.txt` names the commit):
solana-verify 0.5.2 in the pinned image built both programs from a fresh clone of `HEAD`, and
each output is byte-identical to the `anchor build` artifact the tests ran on. The hashes:

| Program | sha256 of the `.so` file | executable hash (trailing zeros trimmed; the on-chain comparison) | bytes |
|---|---|---|---|
| `shortwin.so` | `bbef107570e8eac229911f0b59ba22b48796674605e290e3921c66727817f289` | `3214661b4778a5744ef30b1e1a70c90987185d22ab9c857597c5fbd42c21ee09` | 1,575,216 |
| `complete_curve.so` | `f3a47201738fd72d4dd02f756ba3c17f241dc2bedde1c9d25f63eccaaafc6d42` | `2f2ef7da92c7b8dffc57bd7ae9bf33f64c8dbc09dc552c401742ad85f45382ab` | 122,264 |

- Both ELFs have `e_flags` 0x3 (SBPF v3) and carry exactly one `security.txt` (§10).
- Program batch 4 (PUMP-2026-10-1: PumpSwap's fee buckets) changed `shortwin` only; at batch 3
  (`391aed3`) it was `c4398d01…39ea` / `42f476fe…200b`, 1,574,648 B. `complete_curve` is
  unchanged since batch 3. ARCH §9 records both.
- The executable hash is what `solana-verify get-program-hash` and verify.osec.io compare with
  the deployed ProgramData. Verifying the deployment on chain is M11's (ARCH §9 M11).
- At the skeleton base `e857a2a` the hashes were `00011844…e1a1` / `117392ce…e469` and
  `1970661a…2bd` / `8cbeaea8…6f43`. Batches 1–3 changed both programs (`security.txt` and the
  version), and `shortwin` also by SQ-G8, M10-8C-1 and the extras denylist; batch 4 changed
  `shortwin`'s PumpSwap adapter.

---

## 1. The protocol on one page

short.win is permissionless peer-to-peer token lending on Solana, built for shorting memecoins
(SPEC §1).

- **Lenders** deposit tokens into a per-mint market and earn rent (SPEC §2.4).
- **Borrowers** open a short in one atomic transaction (SPEC §2.1). They post SOL margin, and
  the program borrows `q` tokens from the market's `pool_vault` and sells them on the token's
  venue. The proceeds and the margin stay on the position account.
- **Exits** buy the tokens back on the venue and return them to the vault:
  - close (by the borrower);
  - liquidation (permissionless, when the position is under its CR, SPEC §5);
  - expiry (at the tier's maximum duration).
- **SOL settlement.** When a buyback is impossible or unsafe, a position settles in SOL to the
  lenders under conditions S1–S3 (SPEC §5.3, ECON §8.6).

**Venues.** Every venue is a constant-product pool, priced with exact on-chain math (ARCH §0,
§4):

| Venue | Used for |
|---|---|
| pump.fun bonding curve | coins still on the curve |
| PumpSwap | the canonical pool after graduation |
| Raydium AMM v4 and CPMM | other tokens; the program adopts a pool by a permissionless rule, ARCH §2.6 |
| `None` | list-only markets (deposits and withdrawals only) |

A market moves between venues on graduation and on Raydium pool switches (ARCH §2.5).

**Key mechanisms:**
- **One external signer per market.** A system-owned, zero-data `trader` PDA is the only key
  the program signs with towards a venue. Every leg first normalises the trader's state,
  because anyone can donate to it (ARCH §2.4).
- **Manipulation resistance.** The protocol observes prices in per-second buckets. It keeps an
  EMA with an 8-entry ring, a liquidation persistence rule, and a fixed 2 % bound over a
  manipulation-resistant reference on every keeper buyback. Every settlement decision must
  come out the same on a "settlement box" of EMA and ring medians (ARCH §0 point 3, ECON §5).
- **Lender accounting.** Rent accrues continuously, with O(1) indices. One-off lender income
  vests over 6 h. Withdrawal locks expire with the positions they stand for (ECON §9).
- **Admin.** Governance changes parameters through a timelock (48 h by default) and only
  within compiled bounds. The Emergency multisig has no timelock: it can pause opens, disable
  a venue for opens, revalidate a venue after an upgrade, set a route-broken flag, publish
  bounded extra-account templates, and clear (never set) a keeper coverage pause. **No admin
  instruction can move pool or position funds** (I4; ARCH §3.1 "What admins cannot do").
- **Venue guard.** A permissionless latch records a venue program upgrade. New opens on that
  venue are refused until the Emergency multisig revalidates it; exits never check the guard
  (ARCH §3.1).
- **Keepers.** Anyone can run the exits and the price cranks. The in-house keeper covers
  markets with open positions, within a global daily budget. A market it cannot cover has its
  opens paused through the `coverage_authority` (degraded mode; ARCH §7.1, ECON §5.8).
- **`complete_curve`.** A stateless helper that completes a pump curve inside a straddle
  bundle. It has its own signer, because short.win never forwards a user's signature to a
  venue (ARCH §5.4).

---

## 2. Scope

The in-scope file list is [`docs/audit/scope.txt`](audit/scope.txt). Regenerate it at the tag
with `python3 scripts/audit_scope.py write`. `python3 scripts/audit_scope.py check` fails in
two cases:
- the list and `git ls-files` differ;
- a tracked file under `programs/`, `crates/`, `services/` or a Cargo workspace member is
  in no unit and not on the script's explicit `EXCLUDED` list (listed at the end of
  `scope.txt`).

So a new program, crate or service fails `check` until a reviewed edit of the script puts it in
a unit or excludes it with a reason. On this tree WP-7's `crates/squads-v4-ix` and
`services/ops` (`shortwin-gov`) are classified as the `governance` context unit.

**What is in scope** (owner decisions of 2026-10-02, Q2 and "Audit keeper scope"): the on-chain
core and the keeper's core subset (its coverage budget, liquidation and settlement duties, and
transaction building; ≈ 4.7k non-test nLOC). Line counts on this tree (`m10/batch4`; release ELF
`bbef1075…`):

| Class | Unit | `.rs` files | tokei 14.0.0 "code" | non-test nLOC |
|---|---|---|---|---|
| core | `programs/shortwin` (without the TEST-ONLY `venues/mock.rs`) | 70 | 16,066 | 13,205 |
| core | `programs/complete_curve` | 1 | 293 | 213 |
| core | `crates/shortwin-math` (`src/`) | 25 | 10,158 | 6,214 |
| core | `crates/shortwin-venue-state` (`src/`, without the TEST-ONLY `mock_cpamm.rs`) | 10 | 3,772 | 1,636 |
| core | build pins: `Cargo.toml`, `Cargo.lock`, `Anchor.toml`, `rust-toolchain.toml` | — | — | — |
| **core** | **total** | **106** | **30,289** | **21,268** |
| **keeper** (in scope) | keeper core: exit and crank decisions, coverage and budget, the instructions and account lists it sends (`decide`, `coverage`, `budget`, `fees`, `graduation`, `ixs`, `venue`, `view`, `pda`, `sweep`, `monitor`, `pool_monitor`; `services/common` `tx`, `chain`) | 14 | 7,759 | **4,692** |
| context | keeper rest: the loop (`keeper.rs`, 3,231 non-test nLOC), config, metrics, alerts, the RPC backend, the ALT and v1-probe tools | 12 | 6,316 | 5,821 |
| context | governance: `shortwin-gov` (`services/ops`) and its Squads v4 encoders (`crates/squads-v4-ix`); host-only, never on-chain | 16 | 4,661 | 4,124 |
| out | TEST-ONLY mock venue code (`venues/mock.rs`, `mock_cpamm.rs`, `tests/mock_cpamm`) | 6 | 1,422 | 1,188 |

Since the M10 skeleton (`e857a2a`: core 21,117 nLOC), the three program batches added 126
non-test nLOC to the core: `security.txt` and the AUD-10 lints (batch 1), SQ-G8, M10-8C-1 and
the version (batch 2), and the extras denylist (batch 3). The keeper core is unchanged since
the skeleton; batch 3's `DeniedAccount` fallback is in `keeper.rs` (keeper rest).

**How the lines were counted:**
- **tokei "code"**: tokei 14.0.0 over each unit's `.rs` files (`tokei <files> -t Rust`, the
  file list from `audit_scope.UNITS`). It counts non-blank, non-comment lines and keeps the
  inline `#[cfg(test)]` modules.
- **non-test nLOC**: `python3 scripts/audit_scope.py nloc`. It drops every `#[cfg(test)]` item,
  then counts the same way. The item's end is found by counting braces outside string, char
  and comment text. The script warns when a braced item does not close at its attribute's
  indentation; it gives no warning on this tree.
- The M10 audit-preparation research counted ≈ 21.4k on-chain at `e857a2a` (shortwin 13,702
  with the mock adapter; math 6,193; venue-state 1,686). The script does better on two files:
  - `mint_filter.rs`: the research kept a test module whose attribute spans several lines
    (+263 lines);
  - `u256.rs`: the research kept a test-only `static` (+3 lines).

  It also excludes the mock adapter.
- The research's "keeper subset ≈ 4.8k" had no file list. The keeper-core unit above (4,692)
  is the subset the owner put in scope.

**Out of scope** (read for context only):
- the frontend (`apps/web`), the indexer and API (`services/indexer*`), the generated client
  (`clients/js`) and the WASM facade (`crates/shortwin-math-wasm`, `packages/math-wasm`);
- the keeper outside its core subset, and the governance tool `shortwin-gov` with
  `crates/squads-v4-ix` (the `context` units above; members review and sign in the Squads app,
  the tool only stages and verifies: owner decision of 2026-10-03);
- the tests (`tests/**`, `crates/*/tests`, `services/*-litesvm`), the simulations
  (`docs/design/sim`) and the scripts;
- the external programs. short.win calls or reads them, and the fork tests run their dumped
  mainnet ELFs (`tests/fixtures/manifest.json`):
  - pump.fun, PumpSwap and pump-fees;
  - Raydium AMM v4 and CPMM;
  - SPL Token (p-token), Token-2022, the ATA program and spl-memo;
  - the Pyth SOL/USD feed;
  - Squads v4 (trusted, not reproduced: KI-59).

  Their pins (deploy slots, trimmed ELF hashes, sources) are in `tests/fixtures/README.md` and
  ARCH §1.1. M10 added the CPMM redeploy (`raydium_cpmm_2026_10`, slot 452,376,460), the
  mainnet Token-2022 (v11.0.0, `7ea94a02…`) and spl-memo v3 (`f520eaf0…`), and Squads v4
  (slot 302,582,236, `d4866083…2fac`), and the pump venues' 2026-10-02 redeploys
  (`pump_programs_2026_10`: pump 452,654,932 `b0b777c6…`, PumpSwap 452,654,882 `01557035…`,
  pump-fees 452,655,002 `f166c296…`; the `VenueGuard` baseline `VENUE_GUARD_SLOTS_2026_10_03`).
  `python3 scripts/dump-fixtures.py check --live` reports
  a pinned program that has moved on mainnet.

**The TEST-ONLY code is compiled out of the release ELF:**
- The mock venue exists only under the Cargo feature `test-venue`: 35 `cfg` sites in 6 files
  (`constants.rs`, `engine/tier.rs`, `instructions/create_market.rs`, `state/market.rs`,
  `venues/mod.rs`, and venue-state's `lib.rs`). Two more `cfg`s keep `security.txt` out of the
  test-venue (and `no-entrypoint`) builds of both programs.
- That feature goes only into a separate build, `scripts/build-test-venue.sh`, written to
  `target/test-venue/`.
- CI's anchor-build job (A49) checks that:
  - neither release ELF (`shortwin.so`, `complete_curve.so`) embeds the mock's program id;
  - the release `shortwin.so` lacks the marker `shortwin:test-venue:mock-cpamm`. The marker
    is a `shortwin` constant (`constants.rs`), and `complete_curve`'s `test-venue` feature is
    a no-op, so CI does not grep `complete_curve.so` for it;
  - the test-venue `shortwin.so` contains both, which shows that the checks can see them;
  - the same checks run on the `verifiable-build` job's artifacts.
- Auditors should still confirm that no `test-venue` path reaches release behaviour.

---

## 3. Reading order

1. `README.md`, then SPEC §1–§11 (the product rules; §14 is the decision log).
2. ARCH §0 (summary), §2.3–§2.5 (accounts, funds and signer isolation, the venue state
   machine) and §3.0 (the common instruction structure and the remaining-accounts
   convention).
3. The program, in this order:
   1. `programs/shortwin/src/lib.rs`, the instruction list;
   2. `state/` (`config.rs`, `market.rs`, `position.rs`, `vault.rs`, `layout_asserts.rs`);
   3. `engine/prologue.rs`, then `engine/observe.rs`, `engine/guard.rs` and `engine/route.rs`;
   4. `instructions/open_short.rs`, `close_short.rs`, `liquidate.rs`, `expire.rs`,
      `sol_settle.rs`;
   5. `engine/waterfall.rs`, `engine/bounds.rs`, `engine/caps.rs` and `engine/lender.rs`;
   6. the lender instructions (`deposit.rs`, `withdraw.rs`, `claim_sol.rs`, `cover_deficit.rs`,
      `sync_lender.rs`);
   7. the venue adapters (`venues/mod.rs` and one file per venue);
   8. pool selection (`propose_pool.rs`, `execute_pool_switch.rs`, `engine/pool_select.rs`);
   9. the admin instructions (`initialize_config.rs`, `propose_config.rs`, `apply_config.rs`,
      `cancel_config.rs`, `set_*.rs`, `flag_venue_upgraded.rs`, `revalidate_venue.rs`,
      `sweep_*.rs`);
   10. `mint_filter.rs`.
4. `crates/shortwin-math` (its README lists the modules; ECON §2 gives the numeric
   conventions, ECON §3–§9 the formulas) and `crates/shortwin-venue-state` (the byte-level
   readers; ARCH §4.1).
5. `programs/complete_curve/src/lib.rs` together with ARCH §5.4 (the straddle bundle).
6. If the keeper is in scope: `services/keeper/src/lib.rs`, then `decide.rs`, `view.rs`,
   `ixs.rs` and `venue.rs`, then `keeper.rs` (ARCH §7.1).
7. The invariant hooks (`tests/litesvm/src/invariants.rs` and `invariants/`; ARCH §8.3). They
   are the executable statement of what must always hold.

---

## 4. Trust and threat model

**Roles.** The owner's layout (decision Q4 of 2026-10-02; ARCH §2.3.2, §3.1, §7.4, §8.5). The
two Squads v4 multisigs are created on mainnet in M11 with the members' keys, so their
addresses are not known yet; `shortwin-gov create-multisigs` derives every vault from the
layout file (`services/ops/governance.example.toml`), and `squads_gov_it` builds this exact
layout on the dumped mainnet Squads ELF.

| Multisig | Squads v4 config | Its vaults' roles in short.win |
|---|---|---|
| **Governance** | 3-of-5, `time_lock` 172,800 s (48 h); autonomous (no config authority), no spending limits, rent collector its own vault 0 | vault 0: the upgrade authority of `shortwin` and `complete_curve`, and `governance`; vault 1: `fee_recipient` |
| **Emergency** | 2-of-4, `time_lock` 0; autonomous, no spending limits | vault 0: `pause_authority` |
| (no multisig) | the keeper's hot key | `coverage_authority`, set at `initialize_config` or by a timelocked `Authorities` change |

**SQ-G8 (built in batch 2).** The program refuses a configuration in which two of
`governance`, `pause_authority`, `fee_recipient` and an installed `coverage_authority` share a
key (`config::check_authorities`, `AuthoritiesNotDistinct` = 6105; an unset coverage key is
skipped). It is checked in `initialize_config` and, for an `Authorities` change, at
`propose_config` and again at `apply_config`.

| Role | Key | Can | Cannot |
|---|---|---|---|
| Upgrade authority | Governance vault 0, after the deployer moves it with the checked `SetAuthorityChecked` (staged by `shortwin-gov`; the deployer runs `initialize_config` once, before) | upgrade `shortwin` and `complete_curve` after the 48 h Squads time lock | — (the upgrade is the trust assumption; ARCH §8.5, KI-45, KI-62) |
| `governance` | Governance vault 0 | `propose_config` (timelocked, within compiled bounds, `params::validate` + `validate_econ`), `cancel_config`, `set_pause`, `set_route_broken`, `set_venue_extras` (pins `stable`) | move funds; change a venue program id; skip the timelock |
| `pause_authority` | Emergency vault 0 | `set_pause`, `disable_venue`, `revalidate_venue(venue, expected_slot)`, `set_route_broken`, `set_venue_extras`, `cancel_config` (not `Authorities` or `Timelock` changes), clear (never set) `paused_coverage` | move funds; re-enable a venue (timelocked); veto an upgrade (KI-62) |
| `coverage_authority` | the keeper's hot key | set or clear one market's `paused_coverage` (opens only) | anything else (I4: byte for byte) |
| `fee_recipient` | Governance vault 1 (no spending limits) | receive `sweep_protocol_fees` | — |
| Anyone | — | `crank_price`, `crank_sol_usd`, `liquidate`, `expire`, `sol_settle`, `flag_venue_upgraded`, `flag_unsafe_mint`, `sweep_trader`, `sweep_protocol_fees`, `propose_pool`, `execute_pool_switch`, `create_market`, `apply_config` (after the timelock) | change locked position terms; take any amount but the bounded keeper reward |

**Disclosed Squads v4 semantics** (ARCH §8.5; `squads_gov_it` T-SQ-2, -4, -6, -7; known issues
KI-45, KI-59 … KI-63):
- A parameter change takes ≈ 96 h end to end: 48 h of Squads time lock from the threshold
  approval, then 48 h of `timelock_s`. An upgrade waits only for the 48 h Squads time lock.
- An Approved vault transaction **never expires**. It survives a member rotation and a
  threshold increase, and executes with the approvals it had. A time-lock increase delays it
  but does not stale it.
- Governance **cannot cancel reactively** inside the program window: its `cancel_config` lands
  at `not_before` at the earliest. Only the Emergency can cancel a parameter change in time,
  and never an `Authorities` or `Timelock` change.
- The Emergency **cannot veto an upgrade**; its members cannot vote on Governance proposals.
- A refused vault execution stays Approved and can be re-executed.
- `ExtendProgram` is permissionless: an upgrade to a larger ELF needs it first.
- **No spending limits** exist on either multisig (`shortwin-gov verify` checks the config
  authority is unset).
- Members approve and execute **in the Squads app**; `shortwin-gov` only stages and verifies,
  with no hardware-wallet support. The Emergency's worst-case `set_venue_extras` goes out as
  three single-signer transactions (owner decision of 2026-10-03).

**Attacker models** (ECON §12.1):
- **Atomic model.** Bundles inside one transaction or block.
- **Held model.** Prices held for whole seconds with nobody trading against them; this is also
  the exposure in degraded mode.
- **Other actors.** A keeper that sandwiches its own liquidations; lenders running before an
  S1; an LP holder who wraps cranks in deposits or withdrawals; junk listings that grief the
  keeper's budget; and a malicious or broken venue upgrade.

**External dependencies:**
- the venue programs and their admins (ARCH §5.5, §8.5; KI-44);
- the Token programs and memo (U17);
- the Pyth SOL/USD feed, read through `crank_sol_usd` into an oracle cache. Tiers fall back
  to C when the cache is older than `oracle_stale_s` (600 s);
- Squads v4: immutable since slot 302,582,236, trimmed ELF `d4866083…2fac`, IDL 2.1.0; the
  source (`Squads-Protocol/v4@64af733`) is UNVERIFIED and not rebuilt (ARCH §1.1, §12 U34;
  KI-59). The governance tests run this mainnet ELF itself.

What the design relies on and discloses: ARCH §8.5 and the register's §4.

---

## 5. Invariants and where they are checked

The invariants are ARCH §8.3: I1–I19, A26's locked terms, and ECON §13's economic
invariants.

**Where they are checked:**
- **The hooks.** `tests/litesvm/src/invariants.rs` and `invariants/{lender, positions,
  pool_select, confirm, open_terms, venue_view}.rs` run after **every instruction** of every
  LiteSVM test transaction. A transaction of several instructions is replayed prefix by
  prefix, so each instruction is checked against its own pre-state (ARCH §8.3 "The M1
  hooks").
- **The fuzzer.** `tests/fuzz` (ARCH §8.4) runs the same hooks along seeded action sequences:
  six profiles (`mock`, `mixed`, `curve`, `pumpswap`, `amm-v4`, `cpmm`), on the test-venue
  build and the sha-checked mainnet forks. Its extra oracles are:
  - the I10/I17 gates and I4 over the whole VM;
  - I14 omissions and replacements of every `ReadSet` / `TradeExtras` account (`close_short`
    included since M10; an accepted replacement fails the run);
  - the differential exit oracles: every keeper `liquidate` / `expire` recomputed from
    `shortwin-math` on the mock, AMM v4 and CPMM (every lamport, including I11's liveness
    clause), on the live curve (I-E11) and on PumpSwap (decision level);
  - the lender oracles (M10): `claim_sol ×N` in one transaction pays once; a JIT deposit
    around a landed liquidation, expiry or settlement gains at most its shares' part of the
    immediate rent credit; `[withdraw, sol_settle]` stays within the pre-bundle allowance;
  - the deficit tripwire and the second claim pass of the wind-down (M10; KI-53).
- **Mutation testing.** `scripts/mutants.sh` (M7-SEC-6) proves that the hooks catch seeded
  bugs in the open and coverage paths: 7 of 7 killed.
- **Hook mutants** (M10 WP-8d): tests that feed each hook a wrong instruction and expect it to
  fail, for I17 (`invariants_m2_hooks::i17_no_curve_open_while_graduation_halted`), I18 on
  PumpSwap (`pamm_it::hooks::i18_a_*`, `i18_d_*`, `i18_f_*`) and I15's burned-LP sizing and
  tier (`pamm_it::hooks::i15_an_open_is_sized_and_tiered_on_the_burned_lp`).

| Invariant | Statement (short; ARCH §8.3 is normative) |
|---|---|
| I1 | Tokens: `T + ΣB_Q` equals the lender claims; `pool_vault.amount ≥ T` |
| I2 | Rent-exempt floors plus the tracked SOL on the position and `MarketVault` accounts |
| I3 | Position SOL leaves only through close, liquidate, expire or sol_settle |
| I4 | No admin-signed instruction decreases fund balances, except the fee sweep; no venue id changes; `coverage_authority` touches only its flag |
| I5 | Utilisation ≤ `util_cap` after every open |
| I6 | Aggregates (`ΣB_Q`, rent aggregates, `open_positions`, `max_release`, `max_q_hwm`) match the positions |
| I7 | The trader side is empty after every leg; `pool_vault` has no delegate |
| I8 | ECON I-P1–I-P7, lender SOL and locks (claims, pot, vest, allowances) |
| I9 | Every waterfall conserves `V` |
| I10 | Exits never read the oracle, the guard, the pause flags or the route-broken flag as a gate |
| I11 | Keeper buyback spend ≤ `B̂_ref·(1 + δ)`; the liveness clause |
| I12 | S1 / S2 / S3 takes, the box, persistence and the S3 chain |
| I13 | Venue charge within `[B̂ − 5, B̂]`; EMA single-step bounds |
| I14 | Every pricing, route, tier or guard account is present and key-checked |
| I15 | Raydium adoption eligibility and timing |
| I16 | Fee caps in every risk quote; the route chains are sound |
| I17 | No open under the venue guard; the graduation halt |
| I18 | The post-change confirmation (warm-up, guard, seeds) |
| I19 | An open's terms, recomputed differentially from `shortwin-math` |

---

## 6. Known issues

[`docs/audit/known-issues.md`](audit/known-issues.md) is the single register: 65 rows (KI-01 …
KI-65), each with its bound, the decision that covers it and its status, plus the M10
findings and their resolution (its §6). Do not report these as findings unless the stated
bound is wrong or can be exceeded. Summary:

| Section | Rows | What |
|---|---|---|
| §1 | KI-01 … KI-15, KI-52, KI-53, KI-65 | Economic residuals (O19): the 2 % exit bound and the held model, with the real-venue figures accepted on 2026-10-03 (AMM v4 +61.27, CPMM +37.33 / atomic extra +5.34, PumpSwap +10.20 SOL); no liquidation reserve; baskets; settlement push and deferral; degraded-mode S2→S1; the sale-wrapped open's notional (accepted); the run residual; gap risk; creator self-dealing; **M10-8C-1 (fixed)**; the **late-depositor shortfall** (accepted, with the deposit warning at 1 % / 1 SOL); the **rebase-orphan vest** (disclosed; found by the final tree's 24 h fuzz) |
| §2 | KI-16 … KI-29 | Pool switches and third-party LP (O18; ECON §12.12 residuals 1–17), graduation delays |
| §3 | KI-30 … KI-41 | Keeper coverage and budget (O17, OWNER-M7-*), transaction sizes |
| §4 | KI-42 … KI-46, KI-54 … KI-64 | Trust: the venue guard window, halted venues, upgradeable venues and programs (with the pump venues' 2026-10-02 redeploy), admin powers; the **extras-denylist residuals** (batch 3); the **CPMM creator-fee share**; the Squads source; the **Squads semantics** |
| §5 | KI-47 … KI-51 | Platform facts relied on (U16's 40 M limit, U17, U25, U29, log events) |
| §6 | — | M10 findings: M10-8C-1, B2-2, the extras denylist, PUMP-2026-10-1 … -3 (the pump venues' redeploy), the 24 h fuzz findings, FUZZ-M10-1, the WP-1 / WP-10 doc findings, GC-F1 |

**Status by owner decision** (SPEC §14, ARCH §10):
- **Accepted by the owner:** the real-venue held sandwich (KI-01's and KI-02's figures,
  2026-10-03), the late-depositor shortfall as designed with the warning's 1 % / 1 SOL
  thresholds (KI-53, 2026-10-03), B2-2 kept as built (KI-52, 2026-10-03), the extras-denylist
  residuals (KI-54 … KI-57, 2026-10-03), the first-trade venue rent on short pump accounts
  (KI-64, PUMP-2026-10-2, 2026-10-03), the Squads layout (KI-45, 2026-10-02), and the earlier
  rows marked accepted (KI-12, KI-22 … KI-24, KI-27, KI-28's M6R-4, KI-29, KI-30, KI-35).
- **Fixed:** M10-8C-1 (KI-52), the extras denylist hardening (batch 3), PUMP-2026-10-1 (batch 4:
  PumpSwap's fee buckets are not payable SOL), and the harness and doc findings of the
  register's §6.
- **Open** (build default in force; §7): the rest of O19 (KI-03 … KI-11 and KI-65 are disclosed
  but not signed off), O18 (§2) and the M7 keeper items.

---

## 7. Open owner items and their build defaults

Until the owner decides, the build default stays in force. **Program** marks items whose
decision, or whose alternative to the build default, changes program bytecode. After the tag
such a change is a 1.0.x fix batch with the §12 fix policy. Items marked keeper or web change
only that code.

**Decided in M10** (SPEC §14 and ARCH §10 record them with their dates): Q1 CI route, Q2 audit
shape, Q3 security contact, Q4 Squads layout and SQ-G8, Q5/Q6 defaults (2026-10-02);
M10-8C-1, the release version, the keeper scope, calibration data, CI minutes (2026-10-02);
O19's real-venue figures, the late-depositor warning, the extras denylist hardening, Squads
signing (2026-10-03, third round); B2-2 kept as built, the extras-denylist residuals KI-54 …
KI-57 accepted, the deposit-warning thresholds (1 % / 1 SOL) confirmed, the `SECURITY.md` /
`/security` policy defaults kept (2026-10-03, fourth round); PUMP-2026-10-1 fixed and
PUMP-2026-10-2 disclosed after the pump venues' 2026-10-02 redeploys were re-dumped and
revalidated (2026-10-03, fifth round). All the program-affecting ones are built (batches 2, 3
and 4).

| Item | Decision needed | Build default (in force) | Changes program code? |
|---|---|---|---|
| **O19** (the rest) | Sign off the disclosed residuals of known-issues §1 (KI-01, KI-03 … KI-11); the real-venue figures are already accepted | Accept and disclose; no liquidation reserve | No; the alternative (a rent-only liquidation reserve) is **program** |
| **Rebase-orphan vest** (KI-65; O19; FUZZ-M10-24H-A) | Accept the disclosed residual — in a market whose pool rebased to a few raw token units, the vest every real lender forfeits by exiting during it goes to rebase-orphan shares, claimable by nobody (0.054 SOL in the triaged reproducer; the 24 h runs' largest orphan credit, ≈ 0.218 SOL in one wind-down, is not yet classified) — or fix it with on-chain orphan tracking; first, classify that case (KI-65) | Accept and disclose; lenders are told to wait for the market's `vest_end` (ECON §9.5) | The fix is **program** (not built) |
| **O18** with the SPEC §14 "Delegated, proposed" rows **10** (collateral and liquidation by tier), **13** (dynamic rent) and **18** (fee split) | Confirm the PROPOSED parameters and venue rules: ARCH §2.3.3, §10 O18; ECON §14 lists 32 PROPOSED rows | As built | No if accepted; **program** for any rule the owner rejects |
| **O9** (ECON G37, U21) | The calibrated gap frequencies per tier and the pre-committed rule (if the 95 % upper bound of λ_up exceeds a tier's break-even, raise CR_open or rent, or cut caps), after ≥ 30 days of the public-RPC crawl | Uncalibrated Monte Carlo (ECON §11.6, illustrative); the crawl runs | Parameters through governance; **program** only if a value leaves its compiled bounds |
| **M6D residual 16** (ECON §12.12; KI-26) | Accept the margin of a position opened while a deposit is held, or price the open's margin on the burned depth | Accept (margin on the pool as it is) | The alternative is **program** |
| **OWNER-M6SK-1** (KI-28) | The keeper's tx3 records the new pool's first bucket (+9 → +16 s tracking after a ×3 pump) | As built | Keeper core |
| OWNER-M7-1 (KI-41) | The build-alt table as the normative A28 | Adopt; per-market table as fallback | No (ALT) |
| OWNER-M7-2 (KI-31) | (a) accept the free-pool drain, or (d) a per-market hourly cap on liquidity-anomaly coverage | (a) | Keeper core |
| OWNER-M7-4 (KI-37) | Isolated missed seconds and `liq_cover_s` | PROPOSED until production telemetry | Keeper core |
| OWNER-M7-5 | The LiteSVM action-sequence fuzzer instead of Crucible | As built | No (tooling) |
| OWNER-M7-6 (KI-32) | (a) the alerted overdraft, or (b) cap the overdraft | (a) | Keeper core |
| OWNER-M7-7 | `per_market_daily_lamports` | Superseded by OWNER-M7-17 (decision A) | — |
| OWNER-M7-8 (KI-36) | T-CASCADE defaults (12 M / 20 M) and the fee-payer residual | Confirm; separate crank payer not built | Keeper core |
| OWNER-M7-9 (KI-38) | Keeper load: sequential RPC | Accept for the beta | Keeper / ops |
| **OWNER-M7-17 (3)** and its defaults (KI-33, KI-34) | `G` 1 SOL, floor 1 SOL, margin 10,000 bps, tail 2,000 bps; (3) the drainable free pool, (a) accept or (b) confine bursts; (4) idle cadence below the floor | As built, (a) | Keeper core (budget logic and config) |
| **M8: IX-1, IX-11** (INDEXER.md §14) | Add `rent_protocol`, `q` and `ema_price` to the `Liquidate` / `Expire` / `SolSettle` events, and `record_donation` to `EpochRolled`, so the indexer need not derive them | Not added; the indexer derives them | **Program** (IDL, CU, trace): after the tag only as a 1.0.x batch |
| M8: IX-9, IX-10, IX-14, IX-15, IX-18 | Off-chain indexer items (deployment, fee-sharing attribution, harness coverage, versioned decoders) | Open, out of scope | Indexer only |
| FE-21 | Refuse sending-only wallets for writes, or keep the post hoc check | Post hoc check | Web only |
| FE-23 | Source maps: upload to a private error tracker, or publish | Built hidden, not deployed | Web / ops only |
| Other M10 items | The G-H10 reading of a zero-lamport borrower ((A) reaped after the open, (B) emptied inside the exit's transaction; both tested); the fuzzer's PumpSwap weights (`fee_shock` 3, `open_near_cap` 6); whether to publish the private audit report (recommended, before the beta) | As built; publication undecided | No |
| Scope note | `shortwin-gov` and `crates/squads-v4-ix` are out of scope under the owner's scope decision (programs plus keeper core). They prepare the transactions the members sign; add them to the RFQ if the owner wants them reviewed | Out of scope (§2) | No |

**Closed since the skeleton:** the M10 plan's corrections (WP10-PLAN-Q2) and the four items
tagged M10 without a work package: FE-15 is done in M10 (frontend only, no program bytecode);
FE-18 is partly closed (the mock-venue bundle, claim-time sync and venue claim encodings on a
validator; the pump / PumpSwap composite bundle is M11's); FE-22 moved to M11 (it needs
mainnet latency); IX-16's decoder half is closed (GC-7 / GC-F1), its refresher and admin
watch are M11's.

---

## 8. UNVERIFIED status

ARCH §12 is the **normative** UNVERIFIED register. The research notes (`docs/research/*.md`)
are historical: where they disagree with ARCH §12 or the code, ARCH §12 and the code win.

**Status on this tree** (ARCH §12 after the M10 doc sync):

| U | Item | Status | Left for |
|---|---|---|---|
| U1 | PDA `user` under CPI (pump buy, PumpSwap buy) | VERIFIED on the dumped ELFs (M5, M6). A PumpSwap buy by a third-party PDA is unseen on mainnet | — |
| U2, U3, U6, U7, U9, U11, U24 | Non-ATA WSOL, `disable_flags` bits, rent debits, ATA depth, unsigned `claim_cashback`, CPMM exact-out creator fee, trailing accounts | VERIFIED / RESOLVED (M3–M6) | — |
| U4 | Whole-instruction CU, trace and size figures | **VERIFIED (measured)** for every instruction class (`m1_cu`, `m2_cu`, `m3_cu`, `m5_cu`, `m6_cu`, `m7_keeper`), within the PROPOSED limits, re-baselined after each M10 batch | — |
| U5 | Copying a 10 MiB ProgramData | VERIFIED for CU (not metered). Latency and fee are unmeasured | M11 (mainnet) |
| U8, U22, U25 | The straddle bundle's pre-flight and a live block engine; Jito semantics; v1 transactions landing | Fork parts VERIFIED; the live parts **UNVERIFIED**: no mainnet spend before the audit (owner default Q6) | M11 probes |
| U10 | Delegated `pool_vault` sell; direct delivery for Token-2022 CPMM | **VERIFIED** on the dumped ELFs, re-run on the 452,376,460 CPMM ELF | a mainnet example in M11 |
| U12 | Raydium deployed binaries vs public source | **CPMM closed:** the redeploy is `raydium-cp-swap@bdeee23`, rebuilt byte for byte (the manifest's `source.status` text still says UNVERIFIED; ARCH §12 is normative). **AMM v4:** a behavioural match only | — |
| U13, U14 | Raydium admin history; the `open_time` clamp deploy | Keeper monitor; a conservative constant | — |
| U15, U30 | VQR drift; pump's announced upgrade | Third-party LP explains the drift. The upgrade **was deployed on 2026-10-02** (the `pump-rust-client` 0.2.0 release; what its added `.text` does is UNVERIFIED, coverage is behavioural); re-dumped as `pump_programs_2026_10`, every pump suite and the T-LATCH replay re-run on it, the fee buckets read since batch 4 (PUMP-2026-10-1) | Closed for the freeze; a later pump upgrade goes through the addendum (§12) |
| U16 | The per-writable-account limit | **Desk-checked:** 40 M CU on mainnet in Agave master and Firedancer (below); other clients not checked | monitoring |
| U17 | CPI'd memo satisfies Token-2022 MemoTransfer | **VERIFIED on the mainnet build** (Token-2022 v11.0.0 `7ea94a02…`, spl-memo v3) | a mainnet transaction (M11, TMEMO-10) |
| U18 | `declare_program!`, the lockfile resolution, solana-verify vs `anchor build` | **RESOLVED:** byte-identical for both programs (§0) | CI on this tree (§10) |
| U19, U23 | Surfpool template fidelity; the Switchboard shutdown | **Closed: not relied on** | — |
| U20, U21, U28 | Arbitrage of held pumps; gap frequencies; price noise | Uncalibrated. U21: method and public-RPC crawl since 2026-10-02, results pending | ≥ 30 days of tapes (O9); U28 from production |
| U26 | The latch's reaction time | Monitoring; LiteSVM cannot model it | — |
| U27 | Keeper cost inputs | Closed (M7) | — |
| U29, U31, U32, U33 | pump's cap byte; pAMM `claim_cashback` and `init_user_volume_accumulator`; `set_coin_creator`; the LP accounting behind `D` | Pinned on the dumped ELFs | Re-verify on every pump / pAMM upgrade (U30) |
| U34 | The Squads v4 program's source | Trusted, not reproduced (`64af733` probable, not rebuilt; KI-59) | optional rebuild |

**The re-grep** (`grep -rn UNVERIFIED docs programs tests crates services` on this tree) has no
contradiction with ARCH §12. Every marker outside ARCH §12 maps to a row above or is
historical:

| Where | Marker | Maps to |
|---|---|---|
| `docs/research/*.md` (100 lines) | the research notes' own markers | historical; ARCH §12 wins (the M10 doc sync corrected R-TOOL, R-MINT, R-RAY and the venue notes) |
| `docs/design/ECONOMICS.md` (§17, the boost rule, v1 batches) | ECON's own register | U15, U25, U16 (struck through and restated as desk-checked) |
| `docs/DEVELOPMENT.md`, `tests/fixtures/README.md`, `tests/fixtures/manifest.json`, `tests/litesvm/src/fork.rs`, `crates/squads-v4-ix/src/lib.rs` | the CPMM and Squads fixture sources | U12 (CPMM: rebuilt since; the manifest's status text predates it) and U34 |
| `docs/design/FRONTEND.md` FE-3 | browser bundle submission | U22 |
| `services/common/src/rpc.rs` | live bundle submission | U22 |
| `services/keeper-litesvm/tests/scenarios/m7_cost.rs`, `tests/bench/m7_keeper/results.*`, `services/README.md` | the keeper's live-RPC capacity | OWNER-M7-9 (KI-38) |
| `tests/litesvm/tests/graduation_e2e.rs:1188` | how a bot spends a donation | R-VEN §1.4 (not relied on) |
| `crates/shortwin-math/README.md`, `crates/shortwin-math/tests/quotes_mainnet.rs` | five fee branches with no mainnet trade | covered by the reference-model proptests; the re-scan is deferred (below) |
| `tests/bench/cu_math/README.md` | ARCH §1.1's old Agave resolver question | settled (U18) |
| `docs/design/sim/shortwin_sim.py`, `docs/design/sim/legacy/v0_3/shortwin_sim.py` (historical, the v0.3 sim) | the proportional boost rule | U15 |
| `docs/design/ARCHITECTURE.md` outside §12 (the changelog, the conventions, §1.1, §4.7, §5.4, §6.4, §7.1, §8.2, §10, Appendix A) | the conventions' definition, and the register's items where they are used | the §12 rows each cites: U5, U8, U15, U16, U22, U25, U34; the keeper's live-RPC load is OWNER-M7-9 (KI-38) |
| `docs/AUDIT.md`, `docs/audit/*` | this package | this section |

The stale markers the M10 plan named (ARCH lines 598, 917, 1389, 1736, 1804, 1824, 2448 at
`e857a2a`), the two U5 test-file markers and `withdraw.rs`'s U17 marker are gone: no marker is
left under `programs/`.

**AUD-12 cheap checks** (M10 audit preparation; M10 plan review C3):
- **U4 label.** Done (ARCH §12 U4). Every bench has a committed `results.md` from a passing
  run, and the benches fail CI on any figure over its budget.
- **U16, the Firedancer desk check.** Done, read-only:
  - Firedancer at `firedancer-io/firedancer@e8f25f34` (2026-10-01):
    - `src/flamenco/runtime/fd_slot_params.h` sets `max_writable_account_units` to
      24,000,000 and `max_block_units` to 60,000,000.
    - `fd_cost_tracker.c` multiplies both by 100/60 when the feature
      `raise_block_limits_to_100m` (`P1BCUMpAC7V2GRBRiJCNUgpMyWZhoqt3LKo712ePqsz`) is active:
      40,000,000 per writable account.
    - The pack tile takes the replay limits; `FD_PACK_MAX_WRITE_COST_PER_ACCT_UPPER_BOUND`
      is 100 M × 4/10 = 40 M (SIMD-0306), and the lower bound is 12 M.
  - Agave master at `anza-xyz/agave@e9035015` (2026-10-02):
    - `cost-model/src/block_cost_limits.rs` still has `MAX_WRITABLE_ACCOUNT_UNITS` = 24 M.
    - `runtime/src/slot_params.rs` `cost_limits(raise_block_limits_to_100m)` applies the same
      ×100/60, and `Bank::apply_cost_tracker_limits_for_active_features` installs it.
  - On mainnet (getMultipleAccounts at slot 452,488,037) the feature account says the feature
    was activated at slot 435,888,000.
  - Result: the effective limit is **40 M CU per writable account** (100 M per block) in both
    clients, not the 24 M that ARCH §0 [V10] and §6.6 state.
  - Consequences, none a program change (ARCH §0 [V10], §6.6 and ECON §17 now state 40 M):
    - censoring a market costs ×1.67 more than the 24 M assumption gave;
    - the keeper's `market_cu_per_slot` (12 M) is 30 % of the per-account budget;
    - the OWNER-M7-8 payer residual has more room.
- **The five fee branches with no mainnet trade** (`crates/shortwin-math/README.md:326`,
  `quotes_mainnet.rs:426`): **deferred**. A re-scan needs a crawl that looks for curve
  default-creator and per-curve or per-pool `creator_fee_bps` overrides, and CPMM exact-out
  trades with the creator fee on either side. The calibration crawler's decoders (GC-7) can
  flag them; otherwise M11.

---

## 9. Reproduce

These commands are for a Linux host with the pinned toolchain (`docs/DEVELOPMENT.md` "Setup":
rustup, Agave 4.1.2, Anchor 1.2.0, mise for Node and pnpm). The target is 30 minutes after
the toolchain is installed; the times in the comments are this package's gate (§10) on a
3-job build limit, with a warm `target/`.

```sh
# 1. Build both programs (release) and the TEST-ONLY test-venue build; check SBPF v3.
#    A clone has no program keypair: --ignore-keys silences Anchor's "Program ID mismatch"
#    warning (the id comes from declare_id!). Never run `anchor keys sync`.
anchor build --tools-version v1.57 --arch v3 --ignore-keys && scripts/build-test-venue.sh
od -An -t x4 -j 48 -N 4 target/deploy/shortwin.so            # 00000003
# 2. Hashes, compared with §0
sha256sum target/deploy/shortwin.so target/deploy/complete_curve.so
#    and the verifiable build, byte for byte (Docker; --local runs the same image without one)
scripts/verifiable-build.sh --compare target/deploy            # ≈ 4 min (a fresh clone)
# 3. Fixtures: every dumped mainnet account and ELF matches its checksum
(cd tests/fixtures && sha256sum -c SHA256SUMS) && python3 scripts/dump-fixtures.py check
# 4. The math crate (proptests at the CI count)
PROPTEST_CASES=2000 cargo test -p shortwin-math --locked
# 5. The program tests with the invariant hooks (≈ 20 min debug): the engine on the
#    test-venue build, every venue on the release build against the dumped mainnet ELFs
cargo test -p shortwin-litesvm-tests --locked
# 6. The fuzzer's smoke tests and corpus replays, then a short run on every profile
cargo test --release -p shortwin-fuzz --locked
cargo run -p shortwin-fuzz --release -- run --profile all --duration 10m --jobs 4 --out fuzz-out
```

The full gate (G-full in the M10 plan; CI's `ci.yml`) also runs fmt and clippy with
`-D warnings`, the CU benches, the services tests, mutants, `cargo deny` and `cargo audit`.

---

## 10. Evidence

**CI.** `.github/workflows/ci.yml` has these jobs:
- fmt, clippy, test (with `squads-v4-ix`), vectors, deny, wasm, cu-math;
- anchor-build, including A49 (the release-vs-test-venue check) and the release gate
  `check-security-txt.py --forbid-placeholders` on both ELFs;
- verifiable-build (solana-verify 0.5.2 in the pinned image, byte equality with
  anchor-build's artifacts, A49 on its output, the hashes in the job summary);
- program-tests, t-pot, codama (IDL and client diffs, both IDLs), web, web-e2e;
- the benches m1-cu, m2-cu, m3-cu, m5-cu, m6-cu, m7-keeper-bench, and t-size;
- services (with `shortwin-gov`) and indexer;
- fuzz-smoke on every push; fuzz-nightly and mutants are opt-in. A `schedule` run (main only)
  runs them only when the repository variable `FUZZ_NIGHTLY` is `true` (owner decision
  2026-10-02, `4cb7729`). Otherwise they run on `workflow_dispatch`: fuzz-nightly only with a
  `fuzz_duration` input, mutants on every dispatch.

**CI runs:** the first green run on `main` is at `e857a2a` (run 36972628387).
- **The program-final head `4f4849e`:** run 37161377613
  (https://github.com/TodorBonchev/short.win/actions/runs/37161377613, 2026-10-03), green.
  Every job a push runs succeeded (24 of 26; `fuzz-nightly` and `mutants` are opt-in and were
  skipped). `verifiable-build` ran in Docker mode and found both programs byte-identical to
  anchor-build's artifacts, with the hashes of §0: `shortwin` `bbef1075…f289` / executable
  `3214661b…ee09`, 1,575,216 B; `complete_curve` `f3a47201…6d42` / `2f2ef7da…82ab`, 122,264 B.
  `indexer` passed after the runner-disk fix `3d341c8`; the run before it (37130341616, on
  `cbde0ec`) failed only that job, on a full runner disk.
- **The head that carries this file:** pending. The commits after `4f4849e` (`997c730`,
  `945d45a`: `tests/fuzz` and one test-harness doc comment; then this evidence sync, docs
  only) change no program, client or service code. The next push to `main` (owner decision
  Q1: `main` is fast-forwarded after each verified step) runs CI on it; its URL goes here at
  the tag.

**This package's gate** (re-run for program batch 4, 2026-10-03/04, on `m10/batch4` at
`09778df` … `f08426d`: the pump venues' 2026-10-02 redeploys, PUMP-2026-10-1 and -2, then docs
and the bench re-baselines; the program is the same from `e4f516b` on). Every step of CI's jobs,
run locally through the machine's 3-slot wrapper (logs:
`~/.cache/shortwin-work/m10/b4/logs/SUMMARY`):

| Step (CI job) | Result |
|---|---|
| anchor build + `scripts/build-test-venue.sh` | `shortwin.so` `bbef1075…f289`, `complete_curve.so` `f3a47201…6d42`, test-venue `shortwin.so` `b32f85a0…3e5b`, `mock_cpamm.so` `4cf17007…7ab9` |
| A49 (anchor-build) | `e_flags` 0x3 on every ELF; the mock id in neither release ELF and in the test-venue one; the marker only in the test-venue `shortwin.so`; release ≠ test-venue: `A49-OK` |
| security.txt (anchor-build) | the checker's unit tests; both release ELFs valid with `--forbid-placeholders --source-release v1.0.0`; one marker each, none in the test-venue ELF: `SECTXT-OK` |
| fmt; clippy `-D warnings` on the workspace, `-p shortwin`, `-p shortwin-math` | clean |
| lockfiles; `cargo deny` (root, services); `cargo audit` (8 lockfiles, CI's ignore list) | no lockfile change; `advisories ok, bans ok, licenses ok, sources ok`; no vulnerability |
| vectors; T-CONFIG | regenerated, no diff; `config_check.py` OK and `test_config_gate.py` 8 tests |
| wasm | `shortwin-math` and the facade on `wasm32-unknown-unknown`, `cargo check -p shortwin` there; the facade smoke test |
| fixtures; `check --live` | `dump-fixtures.py check`: 265 entries; `SHA256SUMS` OK; **`check --live` at slot 453,037,762: every pinned program and account matches mainnet** (the pump venues on `pump_programs_2026_10`, `VENUE_GUARD_SLOTS_2026_10_03`) |
| `cargo test -p shortwin-math -p shortwin-math-wasm`, then `PROPTEST_CASES=2000 cargo test -p shortwin-math` | 640 passed (310 + 20, then 310 at 2,000 cases); 0 failed |
| `cargo test -p squads-v4-ix` (test) | 13 passed |
| **`cargo test --workspace --locked`** (test, program-tests) | **1,551 passed, 0 failed, 4 ignored** (112 result blocks incl. the doc tests; the whole LiteSVM suite with the hooks, the pump suites on the redeploys), 1,385 s |
| verifiable build (`--local --compare`) | byte-identical for both programs (§0), 284 s |
| codama; client check | both IDLs equal the build's (no IDL change in batch 4); the regenerated client has no diff |
| web: build, lint, typecheck, Vitest | clean; **570 passed, 1 skipped** |
| **web E2E** (all specs, under the E2E lock) | **58 passed** (6.5 min) |
| services: fmt, clippy (workspace, keeper), build | clean |
| **services tests** (services, indexer) | **351 passed, 0 failed, 6 ignored** (33 result blocks; keeper, LiteSVM scenarios, T-OPS, indexer with Postgres) |
| keeper A49 | the release `shortwin-keeper` is free of the mock id; the scenarios binary embeds it |
| m7-keeper-bench; `keeper_cost.py` | 3 passed; the cost model's constants follow the re-measured PumpSwap and curve exits, and its output equals `docs/design/sim/out/keeper_cost.txt` |
| fuzz crate tests (fuzz-smoke) | 24 passed (release: smoke tests and corpus replays) |
| **fuzz, 10 min each**, `--jobs 3`: `--profile all --seed 20261081`; `--profile curve --seed 20261082`; `--profile pumpswap --seed 20261083` | all: 441 runs, 64,676 actions, 866,755 transactions, 1,702,677 hook checkpoints; curve: 597 runs, 77,021 actions, 1,031,838 transactions, 1,917,894 checkpoints; pumpswap: 335 runs, 49,187 actions, 757,773 transactions, 1,428,899 checkpoints; **0 failures, 0 anomalies** in each; the new `fee_buckets` action (v2 PumpSwap trades and the sweeps) landed 241 / 258 / 933 bucket-carrying v2 trades |
| **mutants** (`scripts/mutants.sh`) | **7 / 7 killed**, by the tests and by a 3-minute fuzz each |
| cu-math; t-pot; t-size | 2; 5; 11 passed |
| benches m1, m2, m3, m5, m6 (fmt, clippy, run, re-baselined) | 1, 4, 5, 7, 3 passed: every figure within its budget; m6 (PumpSwap) +120 … +379 CU from the bucket reads, m5's graduated listing +120; m3 unchanged; m1 / m2 only per-run PDA-bump noise (not committed) |
| `python3 scripts/audit_scope.py check` | ok |
| relative links of `docs/`, `SECURITY.md`, `README.md`, the test READMEs | 23 files, 72 relative links, 0 broken (a script checks every relative markdown link and anchor) |

**Which ELF each suite runs on.** Know this before reading test results:
- The **M2 engine suites run on the TEST-ONLY test-venue ELF**
  (`target/test-venue/deploy/shortwin.so`) against the mock venue. That covers every suite
  built on `Env::test_venue` / `mock_market`, for example:
  - `engine_it`, `liquidate_it`, `expire_it`, `add_margin_it`;
  - the settle suites, `caps_boundary_it`, `security_m2`, `attacks_exit_it`;
  - parts of `m7_sec_it` and `switch_confirm_it`;
  - the `mock` and `mixed` fuzz profiles.

  `tests/litesvm/README.md` lists each binary and its build.
- The **fork suites run on the release ELF** (`target/deploy/shortwin.so`) against the dumped
  mainnet venue ELFs: `ray_*`, `pump_*`, `pamm_it`, `graduation_*`, the `curve`, `pumpswap`,
  `amm-v4` and `cpmm` fuzz profiles, and the m3, m5 and m6 benches.
- A finding in mock-only code paths is out of scope (§2). An engine finding reproduced on the
  mock should be re-checked on a fork.

**Measured figures:** `tests/bench/*/results.md`, `trace.md` and `depth.md` (CU, instruction
trace, stack depth); `tests/litesvm/README.md` (the suites and their figures);
`tests/bench/m7_keeper` and `m7_cascade` (keeper cost and T-CASCADE).

**Fuzz:**
- **24 h run 1**, at `eb82a2a`: seed 20261001, 4 jobs, 86,405 s; 84,053 runs, 16,098,571
  actions, 207,549,074 transactions, 318,356,256 hook checkpoints. 4 failures in 3 harness
  signatures (FUZZ-M7-24H-1, -2; fixed in `e857a2a`) and 1 anomaly (FUZZ-M7-24H-3; fixed in
  `22961b0`, the economics are KI-53). Program invariant violations: 0.
- **24 h run 2** (the confirmation), at `4cb7729` (its `shortwin.so` equals `e857a2a`'s and
  `eb82a2a`'s, `00011844…`): seed 20261002, 4 jobs, 86,401.8 s (2026-10-02T10:12:42Z →
  2026-10-03T10:13Z); 72,149 runs, 13,821,419 actions, 178,365,479 transactions, 273,589,118
  hook checkpoints, 72,149 wind-downs. **0 failures, 0 anomalies**: M7's exit criterion
  "fuzz clean ≥ 24 h" is met.
- **24 h run 3** (the final tree), at `4f4849e` (release `shortwin.so` `bbef1075…`; the M10
  program batches changed both programs after run 2's commit): seed 20261004, 4 jobs,
  86,408.5 s (2026-10-03T23:19:34Z → 2026-10-04T23:20Z); 52,272 runs, 10,162,831 actions,
  132,425,511 transactions, 255,722,249 hook checkpoints, 0 anomalies. It failed two of the
  fuzzer's liveness oracles, both harness bugs with the program unchanged:
  - FUZZ-M10-24H-A (6 runs; crash files for 5, as the fuzzer keeps 5 per signature): the
    second claim pass's oracle was stricter than ECON §9.4 / §9.5's I-P6 residue (the
    accumulators' rounding carries and rebase-orphan credits). Fixed in `997c730` with exact
    state-derived bounds, which still catch the program mutants mutL, mutS and mutC. Its
    spec-level residual is KI-65.
  - FUZZ-M10-24H-B (1 run): the wind-down sized its per-second coverage from the config's
    `settle_persist_n` instead of the position's locked one (latent since M7). Fixed in
    `945d45a`.
- **24 h run 4** (the final tree's confirmation), at `945d45a` (the fixed fuzzer; the program
  bytes of `4f4849e`, `bbef1075…`): seed 20261005, 4 jobs, 86,404.9 s
  (2026-10-05T01:38:12Z → 2026-10-06T01:38Z); 52,274 runs, 10,163,360 actions, 132,445,714
  transactions, 255,725,412 hook checkpoints, 52,274 wind-downs. **0 failures, 0 anomalies**:
  M10's "fuzz clean ≥ 24 h on the final tree" is met. The second claim pass left at most 445
  lamports owed and at most 217,508,010 lamports (≈ 0.218 SOL) of `lender_sol` (5 wind-downs
  above the 0.01-SOL dust cap, each within the rebase-orphan shares' realised SOL); it paid
  13,618 claims. That largest orphan credit is run 3's sixth FUZZ-M10-24H-A failure (run 3's
  summary records the same figure, and there any `lender_sol` above the cap failed), which
  got no crash file, so its shape is not yet classified (KI-65); the triaged reproducers'
  0.054 and 0.012–0.100 SOL are examples, not the bound.
- This gate's 10-minute smoke on this tree is in the table above.

**M10 evidence** (ARCH §9 M10 row and §8.2 are normative):
- **CPMM on the live binary** (WP-2): fixture group `raydium_cpmm_2026_10` (slot 452,376,460),
  `raydium-cp-swap@bdeee23` rebuilt byte for byte; every CPMM suite, the m3 / m5 / m6 and M7
  benches and the web E2E validator on it; no adapter change.
- **T-MEMO** (WP-3, U17): `security::sec_7_*` and `t_memo_control_top_level_sibling_check` on
  the mainnet Token-2022 and spl-memo v3 ELFs; the whole LiteSVM suite runs on them.
- **Adversarial suite** (WP-5, WP-8, the CPMM-deferred cases): the M10 matrix's 62 sub-cases
  per venue (ARCH §8.2 table); new binaries and modules `pamm_it::{structure, wsol, admin,
  hooks}`, `ray_it::route`, `pump_it::fees`, `engine_it::zero_lamport`, `route_it::hard_cap`,
  `raydium_cpmm_attacks`, `pumpswap_attacks`, `raydium_cpmm_exits_it`, `lender_real_it`; ECON
  §12 figures per venue in `tests/litesvm/README.md`.
- **Extras denylist** (batch 3): `engine_it::o6::the_denylist_holds_on_every_leg_instruction`,
  `ray_it::structure::the_pool_and_the_borrower_are_denied_extras_on_both_programs`,
  `pump_it::structure::the_curve_and_the_borrower_are_denied_extras`,
  `pamm_it::admin::the_pool_and_the_borrower_are_denied_extras_on_pumpswap`, the three
  `template_aliases_*` tests, and the keeper scenario
  `extras::liquidations_bypass_a_current_set_the_program_denies`.
- **M10-8C-1** (batch 2): `lender_real_it::finding_m10_8c_1_*` (un-ignored, passing).
- **Squads governance** (WP-7): `crates/squads-v4-ix` (pinned to IDL 2.1.0), `squads_gov_it`
  T-SQ-1..9 on the mainnet Squads ELF, `shortwin-gov` with its T-OPS end-to-end tests
  (`services/ops/tests/gov_e2e.rs`).
- **security.txt** (batches 1–2): the values in §0; `check-security-txt.py
  --forbid-placeholders` on both ELFs in the gate; none in the test-venue ELF; the web's
  `/.well-known/security.txt` and `/security` page; `SECURITY.md`.
- **Gap calibration** (WP-9): the method (`docs/research/gap-calibration.md`) and the
  public-RPC crawler (`scripts/gapcal`), crawling since 2026-10-02; results after ≥ 30 days
  (KI-14, §7 O9).

---

## 11. Focus areas

These are where we would look first. They are not limits on the scope.

1. **Remaining accounts.** Every instruction that prices or trades takes its venue accounts as
   a `ReadSet` / `TradeExtras` tail (ARCH §3.0). Check:
   - key checks against market state (I14);
   - the extras templates and the denylist (A41; `set_venue_extras`, `venues/mod.rs`), including
     M10 batch 3's `venues::pool_denylist` (the adopted pool, its reserve vaults and the
     borrower, checked after a template entry is re-derived);
   - account substitution and omission on every venue.
2. **The trader PDA and the vault delegate.** Check the normalisation before each leg
   (donations of lamports, tokens or accounts), and the `ApproveChecked` of exactly `q` to the
   trader with the delegate auto-cleared at 0 (ARCH §2.4, I7).
3. **Venue CPIs:**
   - the sell and buy legs on pump (`buy_v2` / `sell_v2` with the trailing cap byte, U29),
     PumpSwap (`buy` / `sell` with fee ATAs and UVA existence), AMM v4 (`swap_base_in_v2` /
     `swap_base_out_v2`) and CPMM;
   - measuring every leg against the post-normalisation baseline (I13);
   - `complete_curve`'s single CPI.
4. **The observation layer.** Bucket integration, the EMA, the ring and the settlement box
   (`engine/observe.rs`, `shortwin-math` `observe`, `persist`, `settle`). Rounding direction
   at every step (ECON §2).
5. **Exit math and waterfalls.** The fixed-δ bound, the keeper floor `K_s`, S1 / S2 / S3 and
   fair value (`engine/bounds.rs`, `engine/waterfall.rs`, `shortwin-math` `bounds`,
   `waterfall`, `risk`).
6. **Lender accounting.** Entitlement and realised SOL, the pot, vesting (since M10 also the
   rent top-up, `pool::rent_top_up`), the lock calendar, share epochs after a wipe, the
   shortfall debit on current shares (KI-53), and repeated or bundled claims (I8; ECON §9).
7. **Pool selection and graduation.** `propose_pool` and `execute_pool_switch` (eligibility,
   the window, the fast path, the carried-fit rule); the curve → PumpSwap transition and its
   warm-up (I15, I18).
8. **Admin and config:**
   - the timelock and the apply window;
   - `Authorities` changes, the fee-recipient rule and SQ-G8's distinct-authorities check
     (`config::check_authorities`, at `initialize_config`, `propose_config` and `apply_config`);
   - Emergency limits (what it cannot cancel);
   - `revalidate_venue`'s `expected_slot`;
   - the coverage pause and its 5-s clear delay;
   - compiled parameter bounds (`params::validate`, `validate_econ`).
9. **The mint filter** (`mint_filter.rs`, venue-state `mint.rs`). Check Token-2022 extensions,
   the freeze authority and permanent delegate, and that the filter can never block an exit or
   a withdrawal.
10. **Numerics.** The U256 fallbacks, the `as` casts (58 non-test sites in the audit-prep
    scan) and `wrapping_` / `overflowing_` uses, and the zero-copy layouts (`layout_asserts.rs`,
    T-LAYOUT).
11. **Keeper subset** (if in scope). The account lists it builds (`venue.rs`, `ixs.rs`): a
    wrong list is an exit that does not land. Also the exit decision against the program's
    own predicates (`decide.rs`, `view.rs`) and the budget's exit-critical path
    (`budget.rs`).

---

## 12. Logistics and change policy

**Engagement** (owner decision Q2 of 2026-10-02): one full private audit, then a public
contest (Sherlock or Cantina) on the fixed code, then a bug bounty at the beta. The owner
sends the requests for quotes to OtterSec, Neodyme, Zellic and Offside Labs; the RFQ log is
[`docs/audit/README.md`](audit/README.md). The firm, the dates, the 2 h kickoff walkthrough
and the communication channel are recorded there and in
[`docs/audit/findings.md`](audit/findings.md) when booked.

**Freeze:**
- The audit commit is the tag named in §0.
- Between the tag and the report, the in-scope files change only through fix batches.
- `python3 scripts/audit_scope.py check` and the §0 hashes identify any drift.
- `check` passes on this tree: WP-7's `services/ops` and `crates/squads-v4-ix` were
  classified (the `governance` context unit) and `write` was run. Any later new crate,
  program or service is classified the same way before a fix batch's tag.
- The tag is `audit-1` together with `v1.0.0` (owner decision of 2026-10-02); fix batches
  after it are 1.0.x, each with its own §0 hashes.

**Findings.** Each finding is logged in [`docs/audit/findings.md`](audit/findings.md) and fixed
in a program batch, one commit per finding where practical. Each fix comes with:
- a regression test that fails before the fix: a LiteSVM test or a fuzz corpus file;
- the full gate (G-full) green on the fix commit;
- a ≥ 24 h fuzz run on the fix commit;
- the auditors' fix review.

A finding may instead be acknowledged with a written rationale; it then becomes a row in
`known-issues.md`.

**Venue-upgrade addendum.** The venues upgrade on their own schedule (KI-44); CPMM did so on
2026-09-30 / 10-01. If a venue program changes between the tag and the end of the engagement:
1. Re-dump it as a new fixture group, and confirm the drift with
   `python3 scripts/dump-fixtures.py check --live`. The pump venues' 2026-10-02 redeploys went
   through these steps before the freeze (ARCH changelog 73: `pump_programs_2026_10`, the
   T-LATCH replay, PUMP-2026-10-1 fixed in batch 4).
2. Diff its interface and layouts against the previous dump.
3. Re-run the fork suites, benches and fuzz profiles of that venue on the new ELF.
4. If short.win's adapter has to change, the change is an addendum, with its own scope
   (the adapter diff plus the venue's interface diff), its own hashes, and the same fix
   policy.

**Doc changes.** A change to ARCH, ECON or SPEC that alters a bound in `known-issues.md`
updates the register in the same commit.

**Publication.** Open (§7): we recommend publishing the private audit's report after the
fixes and before the beta. The public contest follows the fixes; the bug bounty starts at the
beta (M11). Published reports are listed on `https://short.win/security`, and both programs'
`security.txt` then gains its `auditors` field (a program upgrade).
