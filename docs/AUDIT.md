# short.win audit package

This is the auditors' entry point to short.win's on-chain code. It says what is in scope, how
to build and test the code, what the code relies on, and what we already know about it.

**Status:**
- This is the M10 skeleton (WP-10), written at base commit `e857a2a`.
- `TODO(WP-n)` marks a part that a later M10 work package fills in. The package is final only
  at the audit tag (WP-12), after the owner has reviewed it.
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
| Audit commit / tag | TODO(WP-12): `audit-1`, plus `v<package version>` on the same commit. Both programs are at version 0.1.0 and the repository has no tags yet. The version that `security.txt`'s `source_release` embeds must be fixed before the freeze hash (M10 plan review B7). |
| Skeleton base | `e857a2a` (branch `m1-core`) |
| `shortwin` program id | `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK` (`programs/shortwin/src/lib.rs` `declare_id!`) |
| `complete_curve` program id | `4B5mGykX9EWwaYdbTvsp8qXCUTbc7caQourz16Gat6LJ` (`programs/complete_curve/src/lib.rs`) |
| TEST-ONLY mock venue | `BPAZFPkmVtYEWEZHCzFUns7oybh1VWfAvmu3ofM2mQi7` (`tests/mock_cpamm`; never deployed; out of scope, §2) |
| Toolchain | Anchor CLI and `anchor-lang` 1.2.0; Agave CLI 4.1.2; platform-tools v1.57 (rustc 1.95), SBPF v3; host Rust 1.98.1 (`docs/DEVELOPMENT.md` "Pins") |
| Build | `anchor build --tools-version v1.57 --arch v3` (never a bare `cargo build-sbf`) |
| Verifiable build | solana-verify 0.5.2, image `solanafoundation/solana-verifiable-build@sha256:2e0b78f44ee76612e9260c7c988570c5e14de6fbd93e0ab07115ec7054473b4f`, `--arch v3 --cargo-build-sbf-args="--tools-version v1.57"`. TODO(WP-4): `scripts/verifiable-build.sh` and the CI job `verifiable-build`. |
| Contacts | TODO(owner, Q3): the security mailbox and the auditors' point of contact; see `SECURITY.md` |

**Hashes at the skeleton base `e857a2a`.** These are not the audit hashes: WP-6 changes both
programs (it adds `security.txt`).

| Program | sha256 of the `.so` file | executable hash (trailing zeros trimmed; the on-chain comparison) | bytes |
|---|---|---|---|
| `shortwin.so` | `00011844c95bba6464ee05a6ff1f47016dcd3d0a2e6cb7c7e60eced2a6a8e1a1` | `117392ceccbbb0172e60bd22dea8c953e6b889571e0bd11bf73dc80b3a74e469` | 1,570,536 |
| `complete_curve.so` | `1970661a9fa5edfa3142b5148354725b51d6627eac400e466a027c37118cd2bd` | `8cbeaea894e862a91cd975458b86213616dab3114fb299371fa82010fed96f43` | 121,968 |

Both rows come from a fresh `anchor build --tools-version v1.57 --arch v3 --ignore-keys` of
`e857a2a` (WP-10, 2026-10-02), with ELF `e_flags` 0x3. They are byte-identical to the M10
verifiable-build research run on the same commit (solana-verify 0.5.2 under a bubblewrap shim
of the pinned image, no Docker). TODO(WP-12): the final hashes, with the CI run URL of the
`verifiable-build` job.

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

So a new program, crate or service, for example WP-7's governance tool under `services/`,
fails `check` until a reviewed edit of the script puts it in a unit or excludes it with a
reason. Line counts on the integrated M10 tree (`m10/integration`, release ELF `a5766d13…`).
Since `e857a2a`, WP-6's `security_txt!` block and AUD-10 lints add 13 non-test nLOC to each
program (tokei: +18 `shortwin`, +14 `complete_curve`):

| Class | Unit | `.rs` files | tokei 14.0.0 "code" | non-test nLOC |
|---|---|---|---|---|
| core | `programs/shortwin` (without the TEST-ONLY `venues/mock.rs`) | 70 | 15,876 | 13,122 |
| core | `programs/complete_curve` | 1 | 294 | 214 |
| core | `crates/shortwin-math` (`src/`) | 25 | 10,063 | 6,190 |
| core | `crates/shortwin-venue-state` (`src/`, without the TEST-ONLY `mock_cpamm.rs`) | 10 | 3,697 | 1,617 |
| core | build pins: `Cargo.toml`, `Cargo.lock`, `Anchor.toml`, `rust-toolchain.toml` | — | — | — |
| **core** | **total** | **106** | **29,930** | **21,143** |
| keeper (Q2) | keeper core: exit and crank decisions, coverage and budget, the instructions and account lists it sends (`decide`, `coverage`, `budget`, `fees`, `graduation`, `ixs`, `venue`, `view`, `pda`, `sweep`, `monitor`, `pool_monitor`; `services/common` `tx`, `chain`) | 14 | 7,759 | 4,692 |
| keeper (Q2) | keeper rest: the loop (`keeper.rs`, 3,235 non-test nLOC), config, metrics, alerts, the RPC backend, the ALT and v1-probe tools | 12 | 6,320 | 5,825 |
| out | TEST-ONLY mock venue code (`venues/mock.rs`, `mock_cpamm.rs`, `tests/mock_cpamm`) | 6 | 1,422 | 1,188 |

**How the lines were counted:**
- **tokei "code"**: tokei 14.0.0 over each unit's `.rs` files (`tokei <files> -t Rust`). It
  counts non-blank, non-comment lines and keeps the inline `#[cfg(test)]` modules.
- **non-test nLOC**: `python3 scripts/audit_scope.py nloc`. It drops every `#[cfg(test)]` item,
  then counts the same way. The item's end is found by counting braces outside string, char
  and comment text. The script warns when a braced item does not close at its attribute's
  indentation; it gives no warning on this tree.
- The M10 audit-preparation research counted ≈ 21.4k on-chain (shortwin 13,702 with the mock
  adapter; math 6,193; venue-state 1,686). The script does better on two files:
  - `mint_filter.rs`: the research kept a test module whose attribute spans several lines
    (+263 lines);
  - `u256.rs`: the research kept a test-only `static` (+3 lines).

  It also excludes the mock adapter.
- The research's "keeper subset ≈ 4.8k" had no file list. The keeper-core unit above (4,692)
  is this package's proposal; the owner decides whether the keeper is in scope at all (Q2).

**Out of scope** (read for context only):
- the frontend (`apps/web`), the indexer and API (`services/indexer*`), the generated client
  (`clients/js`) and the WASM facade (`crates/shortwin-math-wasm`, `packages/math-wasm`);
- the tests (`tests/**`, `crates/*/tests`, `services/*-litesvm`), the simulations
  (`docs/design/sim`) and the scripts;
- the external programs. short.win calls or reads them, and the fork tests run their dumped
  mainnet ELFs (`tests/fixtures/manifest.json`):
  - pump.fun, PumpSwap and pump-fees;
  - Raydium AMM v4 and CPMM;
  - SPL Token (p-token), Token-2022, the ATA program and spl-memo;
  - the Pyth SOL/USD feed;
  - Squads v4.

  TODO(WP-1): add the Token-2022, memo and Squads fixture pins.

**The TEST-ONLY code is compiled out of the release ELF:**
- The mock venue exists only under the Cargo feature `test-venue`: 34 `cfg` sites in 6 files
  (`constants.rs`, `engine/tier.rs`, `instructions/create_market.rs`, `state/market.rs`,
  `venues/mod.rs`, and venue-state's `lib.rs`).
- That feature goes only into a separate build, `scripts/build-test-venue.sh`, written to
  `target/test-venue/`.
- CI's anchor-build job (A49) checks that:
  - neither release ELF (`shortwin.so`, `complete_curve.so`) embeds the mock's program id;
  - the release `shortwin.so` lacks the marker `shortwin:test-venue:mock-cpamm`. The marker
    is a `shortwin` constant (`constants.rs`), and `complete_curve`'s `test-venue` feature is
    a no-op, so CI does not grep `complete_curve.so` for it;
  - the test-venue `shortwin.so` contains both, which shows that the checks can see them.
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

**Roles.** TODO(WP-7): the Squads v4 layout the owner records (Q4), with vault addresses.

| Role | Key | Can | Cannot |
|---|---|---|---|
| Upgrade authority | the Governance multisig's vault 0 (Squads v4; Q4 default: 3-of-5, `time_lock` 172,800 s) | upgrade `shortwin` and `complete_curve`; runs `initialize_config` once, from the deployer, before the authority moves | — (the upgrade is the trust assumption, ARCH §8.5) |
| `governance` | the Governance vault | `propose_config` (timelocked, within compiled bounds, `params::validate` + `validate_econ`), `cancel_config`, `set_pause`, `set_route_broken`, `set_venue_extras` (pins `stable`) | move funds; change a venue program id; skip the timelock |
| `pause_authority` | the Emergency multisig's vault 0 (Q4 default: 2-of-4, `time_lock` 0) | `set_pause`, `disable_venue`, `revalidate_venue(venue, expected_slot)`, `set_route_broken`, `set_venue_extras`, `cancel_config` (not `Authorities` or `Timelock` changes), clear (never set) `paused_coverage` | move funds; re-enable a venue (timelocked); veto an upgrade (to disclose, TODO(WP-7)) |
| `coverage_authority` | the keeper's hot key, installed only by a timelocked `Authorities` change | set or clear one market's `paused_coverage` (opens only) | anything else (I4: byte for byte) |
| `fee_recipient` | Q4 default: Governance vault 1 (no spending limits) | receive `sweep_protocol_fees` | — |
| Anyone | — | `crank_price`, `crank_sol_usd`, `liquidate`, `expire`, `sol_settle`, `flag_venue_upgraded`, `flag_unsafe_mint`, `sweep_trader`, `sweep_protocol_fees`, `propose_pool`, `execute_pool_switch`, `create_market`, `apply_config` (after the timelock) | change locked position terms; take any amount but the bounded keeper reward |

Owner question Q4(e) (SQ-G8, a program change) would make the program check that
`governance`, `pause_authority` and `fee_recipient` are pairwise distinct (the M10 plan
review adds `coverage_authority`); TODO(WP-6) if the owner approves.

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
- Squads v4 (immutable at slot 302,582,236). TODO(WP-1/WP-7) for its pins.

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
  six profiles, on the test-venue build and the sha-checked mainnet forks. Its extra oracles
  are the I10/I17 gates, a differential exit oracle, I14 omissions and replacements, and I4
  over the whole VM.
- **Mutation testing.** `scripts/mutants.sh` (M7-SEC-6) proves that the hooks catch seeded
  bugs in the open and coverage paths: 7 of 7 killed.

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

TODO(WP-8d): the I13 doc row in `invariants/positions.rs`, and the new hook mutants (I15,
I17, I18 on PumpSwap).

---

## 6. Known issues

[`docs/audit/known-issues.md`](audit/known-issues.md) is the single register: 51 rows (KI-01 …
KI-51), each with its bound, the decision that covers it and its status. Do not report these
as findings unless the stated bound is wrong or can be exceeded. Summary:

| Section | Rows | What |
|---|---|---|
| §1 | KI-01 … KI-15 | Economic residuals (O19): the 2 % exit bound, the held model, no liquidation reserve, baskets, settlement push and deferral, degraded-mode S2→S1, the sale-wrapped open's notional (accepted), the run residual, gap risk, creator self-dealing |
| §2 | KI-16 … KI-29 | Pool switches and third-party LP (O18; ECON §12.12 residuals 1–17), graduation delays |
| §3 | KI-30 … KI-41 | Keeper coverage and budget (O17, OWNER-M7-*), transaction sizes |
| §4 | KI-42 … KI-46 | Trust: the venue guard window, halted venues, upgradeable venues and programs, admin powers |
| §5 | KI-47 … KI-51 | Platform facts relied on (U16, U17, U25, U29, log events) |

---

## 7. Open owner items and their build defaults

Until the owner decides, the build default stays in force. **Program** marks items whose
decision, or whose alternative to the build default, changes program bytecode. Those must
land before the freeze hash. Items marked keeper or web change only that code.

| Item | Decision needed | Build default (in force) | Changes program code? |
|---|---|---|---|
| **O9** (ECON G37, U21) | Calibrated gap frequencies per tier and the pre-committed rule: if the 95 % upper bound of λ_up exceeds a tier's break-even, raise CR_open or rent, or cut caps (M10 Q5) | Uncalibrated Monte Carlo (ECON §11.6, illustrative) | Parameters through governance; **program** only if a value leaves its compiled bounds. TODO(WP-9) |
| **O18** with the SPEC §14 "Delegated, proposed" rows **10** (collateral and liquidation by tier), **13** (dynamic rent) and **18** (fee split) | Confirm the PROPOSED parameters and venue rules: ARCH §2.3.3, §10 O18; ECON §14 lists 32 PROPOSED rows | As built | No if accepted; **program** for any rule the owner rejects |
| **O19** | Accept the disclosed residuals (known-issues §1) | Accept and disclose; no liquidation reserve | No; the alternative (a rent-only liquidation reserve) is **program** |
| **M6D residual 16** (ECON §12.12; KI-26) | Accept the margin of a position opened while a deposit is held, or price the open's margin on the burned depth | Accept (margin on the pool as it is) | The alternative is **program** |
| **OWNER-M6SK-1** (KI-28) | The keeper's tx3 records the new pool's first bucket (+9 → +16 s tracking after a ×3 pump) | As built | Keeper only |
| OWNER-M7-1 (KI-41) | The build-alt table as the normative A28 | Adopt; per-market table as fallback | No (ALT) |
| OWNER-M7-2 (KI-31) | (a) accept the free-pool drain, or (d) a per-market hourly cap on liquidity-anomaly coverage | (a) | Keeper only |
| OWNER-M7-4 (KI-37) | Isolated missed seconds and `liq_cover_s` | PROPOSED until production telemetry | Keeper only |
| OWNER-M7-5 | The LiteSVM action-sequence fuzzer instead of Crucible | As built | No (tooling) |
| OWNER-M7-6 (KI-32) | (a) the alerted overdraft, or (b) cap the overdraft | (a) | Keeper only |
| OWNER-M7-7 | `per_market_daily_lamports` | Superseded by OWNER-M7-17 (decision A) | — |
| OWNER-M7-8 (KI-36) | T-CASCADE defaults (12 M / 20 M) and the fee-payer residual | Confirm; separate crank payer not built | Keeper only |
| OWNER-M7-9 (KI-38) | Keeper load: sequential RPC | Accept for the beta | Keeper / ops |
| **OWNER-M7-17 (3)** and its defaults (KI-33, KI-34) | `G` 1 SOL, floor 1 SOL, margin 10,000 bps, tail 2,000 bps; (3) the drainable free pool, (a) accept or (b) confine bursts; (4) idle cadence below the floor | As built, (a) | Keeper only (budget logic and config) |
| FE-21 | Refuse sending-only wallets for writes, or keep the post hoc check | Post hoc check | Web only |
| FE-23 | Source maps: upload to a private error tracker, or publish | Built hidden, not deployed | Web / ops only |

**Corrections to the M10 plan (Q2):**
- The plan lists OWNER-M7-17 (3) and OWNER-M6SK-1 among the "program-affecting" items that
  gate the freeze. Both change only keeper code. They gate the freeze only if the keeper
  subset is in scope.
- Program-affecting decisions that the owner-gates list above does not name:
  - **Q3**: the final `security.txt` values. They are bytes in both ELFs.
  - **Q4(e)**: SQ-G8, the distinct-authorities check.
  - **The version bump**: `source_release` embeds the package version (M10 plan review B7).
  - **CPMM adapter changes**: any that WP-2 finds after the CPMM redeploy.

**Items tagged M10 that no work package covers** (M10 plan review C1). The suggestion for each is
WP-11's to record:
- **FE-15**: the WASM facade exports. A frontend refactor, outside the audit; re-tag M11.
- **FE-18**: the post-wipe sweep bundle and the claim-time `sync_lender` on a validator.
  WP-8c's `lender_real_it` (two real wipes) can cover the program side; the browser side moves
  to M11.
- **FE-22**: the slot-age thresholds. They need mainnet latency data; re-tag M11.
- **IX-16**: venue layouts against mainnet. WP-9's decoder parity tests (GC-7) cover the
  decoder part.

---

## 8. UNVERIFIED status

ARCH §12 is the **normative** UNVERIFIED register. The research notes (`docs/research/*.md`)
are historical: where they disagree with ARCH §12 or the code, ARCH §12 and the code win.

TODO(WP-11): reconcile the doc-wide re-grep `grep -rn UNVERIFIED docs programs tests crates
services` with §12. That includes the stale lines the M10 plan names (ARCH 598, 917, 1389,
1736, 1804, 1824, 2448) and the R-TOOL corrections.

| U | Item | Status at `e857a2a` | M10 action |
|---|---|---|---|
| U1 | PDA `user` under CPI (pump buy, PumpSwap buy) | VERIFIED on the dumped ELFs (M5, M6). A PumpSwap buy by a third-party PDA is unseen on mainnet | — |
| U2, U3, U6, U7, U9, U11, U24 | Non-ATA WSOL, `disable_flags` bits, rent debits, ATA depth, unsigned `claim_cashback`, CPMM exact-out creator fee, trailing accounts | VERIFIED / RESOLVED (M3–M6) | — |
| U4 | Whole-instruction CU, trace and size figures | Every instruction class is measured: M1 admin (`m1_cu`), the M2 engine on the mock (`m2_cu`), Raydium (`m3_cu`), the curve (`m5_cu`), PumpSwap (`m6_cu`), the keeper's own transactions (`m7_keeper`). All are within the PROPOSED limits | **Relabel VERIFIED (measured)** (AUD-12; text for WP-11 in the WP-10 report). Re-baselined after each WP-6 batch |
| U5 | Copying a 10 MiB ProgramData | VERIFIED for CU (M3: not metered, 6,032 / 8,440 CU). Latency and fee are unmeasured | The two stale test-file markers are fixed in WP-10 (`tests/litesvm/tests/it/cu.rs`, `tests/bench/m1_cu/src/lib.rs`; M10 plan review C4) |
| U8, U22 | The straddle bundle on a live block engine; Jito semantics | Curve and tx3 VERIFIED on the fork. RPC pre-flight and live submission unverified | Q6(a) probe, else M11 |
| U10 | Delegated `pool_vault` sell; direct delivery for Token-2022 CPMM | Tested in M3 (`ray_it::pda`, including `cpmm_token_2022`), but the §12 row was never relabelled | TODO(WP-2): re-run on the 452,376,460 CPMM ELF, then WP-11 relabels |
| U12 | Raydium deployed binaries vs public source | Behavioural match only. CPMM was redeployed at 452,376,460, probably from branch `feat/creator-fee-share` `bdeee23` (M10 plan review A1); not verified | TODO(WP-2) |
| U13, U14 | Raydium admin history; the `open_time` clamp deploy | Keeper monitor; a conservative constant | — |
| U15, U30 | VQR drift; pump's announced 2026-09-30 upgrade | Third-party LP explains the drift. The upgrade is not deployed (watch) | The latch, and a re-dump on deployment |
| U16 | The per-writable-account limit in clients other than Agave | **Desk-checked in WP-10** (below): 40 M on mainnet in both Agave master and Firedancer | TODO(WP-11): ARCH §0 [V10], §6.6, U16 text |
| U17 | CPI'd memo satisfies Token-2022 MemoTransfer | Passes on the bundled Token-2022 | TODO(WP-3): on the mainnet Token-2022 v11.0.0 ELF (`7ea94a02…`) |
| U18 | `declare_program!`, the lockfile resolution, solana-verify vs `anchor build` | The M10 research shows byte equality at `e857a2a`, and the WP-10 build reproduces the hash | TODO(WP-4) closes it in CI |
| U19, U23 | Surfpool template fidelity; the Switchboard shutdown | Not used by the build | WP-11: close as not relied on |
| U20, U21, U28 | Arbitrage of held pumps; gap frequencies; price noise | Uncalibrated assumptions | TODO(WP-9) (U21, partly U20 and U28); U28 from production |
| U25 | v1 transactions land on mainnet | Tool built (`shortwin-v1-probe`), not run | Q6(a) |
| U26 | The latch's reaction time | Monitoring; LiteSVM cannot model it | — |
| U27 | Keeper cost inputs | Closed (M7) | — |
| U29, U31, U32, U33 | pump's cap byte; pAMM `claim_cashback` and `init_user_volume_accumulator`; `set_coin_creator`; the LP accounting behind `D` | Pinned on the dumped ELFs | Re-verify on every pump / pAMM upgrade |

**UNVERIFIED markers in code** (`grep -rn UNVERIFIED --include=*.rs`) that no work package
owns (M10 plan review C4):
- U5 in `tests/litesvm/tests/it/cu.rs:3` and `tests/bench/m1_cu/src/lib.rs:60`: restated in
  WP-10 as VERIFIED for CU, with latency and fee unmeasured. Comments only.
- `services/common/src/rpc.rs:20` (U22), `services/keeper-litesvm/tests/scenarios/m7_cost.rs`
  (OWNER-M7-9, the live-RPC capacity), `tests/litesvm/tests/graduation_e2e.rs:1188` (R-VEN
  §1.4) and `crates/shortwin-math/tests/quotes_mainnet.rs` (the five fee branches): still
  accurate.
- `programs/shortwin/src/instructions/withdraw.rs:37` (U17): TODO(WP-6), once WP-3 lands.

**AUD-12 cheap checks** (M10 audit preparation; M10 plan review C3):
- **U4 label.** Done (table above). Read-only check: every bench has a committed
  `results.md` from a passing run, and the benches fail CI on any figure over its budget.
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
  - Consequences, all for the doc delta; none is a program change:
    - censoring a market costs ×1.67 more than ARCH §6.6 states;
    - the keeper's `market_cu_per_slot` (12 M) is now 30 % of the per-account budget;
    - the OWNER-M7-8 payer residual has more room.
- **The five fee branches with no mainnet trade** (`crates/shortwin-math/README.md:326`,
  `quotes_mainnet.rs:426`): **deferred**. A re-scan needs a crawl that looks for curve
  default-creator and per-curve or per-pool `creator_fee_bps` overrides, and CPMM exact-out
  trades with the creator fee on either side. WP-9's crawler (GC-7 decoders) can flag them;
  otherwise M11.

---

## 9. Reproduce

These commands are for a Linux host with the pinned toolchain (`docs/DEVELOPMENT.md` "Setup":
rustup, Agave 4.1.2, Anchor 1.2.0, mise for Node and pnpm). TODO(WP-12): time them on the tag;
the target is 30 minutes after the toolchain is installed.

```sh
# 1. Build both programs (release) and the TEST-ONLY test-venue build; check SBPF v3
anchor build --tools-version v1.57 --arch v3 && scripts/build-test-venue.sh
od -An -t x4 -j 48 -N 4 target/deploy/shortwin.so            # 00000003
# 2. Hashes, compared with §0
sha256sum target/deploy/shortwin.so target/deploy/complete_curve.so
#    TODO(WP-4): scripts/verifiable-build.sh --local --compare target/deploy
# 3. Fixtures: every dumped mainnet account and ELF matches its checksum
(cd tests/fixtures && sha256sum -c SHA256SUMS) && python3 scripts/dump-fixtures.py check
# 4. The math crate (proptests at the CI count)
PROPTEST_CASES=2000 cargo test -p shortwin-math --locked
# 5. The program tests with the invariant hooks (≈ 15 min debug): the engine on the test-venue
#    build, every venue on the release build against the dumped mainnet ELFs
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
- fmt, clippy, test, vectors, deny, wasm, cu-math;
- anchor-build, including A49, the release-vs-test-venue check;
- program-tests, t-pot, codama (IDL and client diffs, both IDLs), web, web-e2e;
- the benches m1-cu, m2-cu, m3-cu, m5-cu, m6-cu, m7-keeper-bench, and t-size;
- services and indexer;
- fuzz-smoke on every push; fuzz-nightly and mutants are opt-in. A `schedule` run (main only)
  runs them only when the repository variable `FUZZ_NIGHTLY` is `true` (owner decision
  2026-10-02, `4cb7729`). Otherwise they run on `workflow_dispatch`: fuzz-nightly only with a
  `fuzz_duration` input, mutants on every dispatch.

TODO(WP-4) adds the `verifiable-build` job, and TODO(WP-6) a `security.txt` step.
TODO(WP-12): the green run URL on the tag (Q1: how `m1-core` gets CI).

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
- The M7 24 h run (`~/.cache/shortwin-work/fuzz24`) runs on a harness from before `e857a2a`.
  Its three failures are harness bugs, fixed in `e857a2a` (FUZZ-M7-24H-1, -2; M10 plan review B6).
- TODO(WP-12): a ≥ 24 h run on the final tree with 0 failures, with its commit and
  `summary.json`.

**M10 additions:** TODO(WP-2 … WP-9):
- WP-2: CPMM on the live binary;
- WP-3: T-MEMO on the mainnet Token-2022;
- WP-5 and WP-8: the adversarial matrix re-run with its new test binaries;
- WP-7: Squads `squads_gov_it` T-SQ-1..9;
- WP-6: `security.txt` (`query-security-txt` output);
- WP-9: the calibration report.

---

## 11. Focus areas

These are where we would look first. They are not limits on the scope.

1. **Remaining accounts.** Every instruction that prices or trades takes its venue accounts as
   a `ReadSet` / `TradeExtras` tail (ARCH §3.0). Check:
   - key checks against market state (I14);
   - the extras templates and the denylist (A41; `set_venue_extras`, `venues/mod.rs`);
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
6. **Lender accounting.** Entitlement and realised SOL, the pot, vesting, the lock calendar,
   share epochs after a wipe, and repeated or bundled claims (I8; ECON §9).
7. **Pool selection and graduation.** `propose_pool` and `execute_pool_switch` (eligibility,
   the window, the fast path, the carried-fit rule); the curve → PumpSwap transition and its
   warm-up (I15, I18).
8. **Admin and config:**
   - the timelock and the apply window;
   - `Authorities` changes and the fee-recipient rule;
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

**Engagement.** TODO(owner, Q2): the firm, dates, the kickoff walkthrough (2 h), and the
communication channel. The RFQ log is [`docs/audit/README.md`](audit/README.md).

**Freeze:**
- The audit commit is the tag named in §0.
- Between the tag and the report, the in-scope files change only through fix batches.
- `python3 scripts/audit_scope.py check` and the §0 hashes identify any drift.
- Before the tag, WP-12 classifies every path that `check` reports as unclassified. That
  covers each new crate, program or service since `e857a2a`, such as WP-7's `services/ops`.
  Each one goes in a unit or in `EXCLUDED` with a reason, and then `write` is run.

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
   `python3 scripts/dump-fixtures.py check --live` (TODO(WP-1)).
2. Diff its interface and layouts against the previous dump.
3. Re-run the fork suites, benches and fuzz profiles of that venue on the new ELF.
4. If short.win's adapter has to change, the change is an addendum, with its own scope
   (the adapter diff plus the venue's interface diff), its own hashes, and the same fix
   policy.

**Doc changes.** A change to ARCH, ECON or SPEC that alters a bound in `known-issues.md`
updates the register in the same commit.

**Publication.** TODO(owner, Q2): reports published after the fixes and before the beta
(recommended). A contest or bug bounty follows the fixes (M11).
