# short.win — Architecture and Implementation Blueprint

*Design document **v0.4.1** ("v4.1", fourth review round), 2026-09-27. Supersedes v0.4 of the same day. Baseline: [`docs/SPEC.md`](../SPEC.md) v0.3, including owner decisions **D1–D4** (owner-approved; this document does not change it). Economics: [`docs/design/ECONOMICS.md`](ECONOMICS.md) **v0.4.1** and its reference models [`sim/pool_model.py`](sim/pool_model.py) v0.4.1, [`sim/shortwin_sim.py`](sim/shortwin_sim.py) v0.4.1, the attack scripts in [`sim/attacks/`](sim/attacks/), the parameter gate [`sim/config_check.py`](sim/config_check.py) and the keeper cost model [`sim/keeper_cost.py`](sim/keeper_cost.py). Research inputs: every file in [`docs/research/`](../research/). Review input: the 33 first-round findings `C0-1` … `C1-15`, the 27 second-round findings `V1-1` … `V2-14`, the 25 third-round findings `R3A-1` … `R3A-13` and `R3B-1` … `R3B-12` with their two simplification lists, and the 20 fourth-round findings `R4A-1` … `R4A-10` and `R4B-1` … `R4B-10`; [Appendix A](#appendix-a-review-resolution) records the outcome of each one.*

This is the blueprint that the protocol, application and security work is built from. It turns SPEC, ECONOMICS and the research notes into accounts, instructions, byte layouts, checks, budgets, tests and milestones. You should be able to implement from this document without re-reading the research. Derivations are in ECONOMICS and in the research notes, and are referenced by section.

**Where ECONOMICS and this document meet, they agree exactly** (both v0.4.1). Parameter names, defaults, bounds and formulas are the ones in ECON §15 and ECON §2–§10. Numerics (type, scale, rounding, order of operations) are ECON §2.1, which is normative for the math crate. Architecture-only items (no economic effect) are marked as such. Where this document places a piece of ECON's logical market state in a particular account, it says so (for example the lock state on `MarketVault`, §2.3.6).

## Changelog v0.4 → v0.4.1 (fourth review round)

The fourth review (`R4A-1` … `R4A-10`, `R4B-1` … `R4B-10`) re-checked the v0.4 fixes. Each finding was reproduced before it was fixed; ECON v0.4.1 changelog #74–#89 has the economic evidence and Appendix A ("Review resolution — round 4") the outcome of every finding.

| # | Change | Finding | Where |
|---|---|---|---|
| 25 | **`liquidate` reserves nothing in `max_in`**: `max_in = min(⌊B̂_ref·(10⁴ + δ)/10⁴⌋, V − K_s − ert)` (SPEC §5.2). v0.4's rent + penalty reserve (row 2) blocked honest liquidations in genuine fast pumps, and v0.3's rent-only reserve does the same once rent due exceeds `K_s` (aged positions). `expire` keeps rent and the expiry fee senior; `exec_tol_bps` (ε) now enters only the expiry bound. New invariant I-E11 (liveness) | R4A-1, R4B-1 | §3.6, §3.7, §5.2, I11 |
| 26 | **Liquidation reference also capped at `lo_x·(1 + κ_eff)`**, so a pushed-and-frozen `P_ema` cannot lift the bound | R4B-2 | §3.6 |
| 27 | **Settlement box on bucket extremes**: `RingEntry` stores the integrated bucket's `lo, hi, llo, lhi` instead of `obs, lobs` (136 B; `Market` 3,104 B); `P_lo = min(P_ema, lower median of lo)`, `P_hi = max(P_ema, upper median of hi)`, likewise `L`. Push-then-freeze no longer moves it; paid per-second widening remains a deferral (disclosed) | R4B-2, R4A-9 | §2.3.4, §3.8, §4.4, §4.5 |
| 28 | **FV's `p'` decided on each box corner** (never on the raw legs); the S2/S3 immediate part is `min(x, ⌊Q·P_ema/WAD⌋ + R_due)`, the rest vests (`waterfall::settle` takes `Q`, `P_ema`, `R_due`; no `B̂_FV` reconstruction) | R4A-3, R4B-6, R4A-5 | §3.8, §4.5, §5.2 |
| 29 | **Lock expiry calendar** on `MarketVault` (zero-copy): 128 day buckets of lock terms; `lock_until` rounded up to its day's end; the aggregate drops a bucket when its day ends, so an idle lender's expired lock stops counting for everyone at once. Lock terms floored. I-P4 asserted on the untouched state (T-POT, Crucible) | R4A-2, R4B-3 | §2.3.6, §3.3, I8 |
| 30 | **`max_release`** (replaces `max_deadline`): the latest `deadline + max_migration_pause_s + grad_grace_s + s3_wait_s` (snapshot values) of any position opened; `lock_until = ⌈(max(now, max_release) + lock_grace_s)/1 d⌉·1 d` | R4A-8, R4B-8 | §2.3.4, §3.3, §3.4 |
| 31 | **Value-weighted vest end**; the "last lender" flush only at `S → 0`; an exit forfeits the unreleased vest (disclosed) | R4B-5, R4B-4 | §3.3 |
| 32 | **Deposits wait for the post-wipe sweep** (`Market.flags` bits `wipe_trader_swept`, `wipe_rewards_claimed`; `WipeSweepPending`) | R4A-7 | §2.3.4, §3.3, §3.9, §3.11 |
| 33 | **Venue latch hardened**: `revalidate_venue(venue, expected_slot)`; `flag_venue_upgraded` refreshes the recorded slot while the bit is set; the `PumpCurve` open mask includes pAMM and each Raydium mask both Raydium programs | R4B-7 | §2.3.2, §3.1, §3.4 |
| 34 | **Keeper batches are v1 transactions** (a v0 transaction fits 4 PumpSwap / CPMM or 5 curve cranks) | R4B-9 | §6.4, §7.1 |
| 35 | **Compiled bounds re-derived** for every parameter with a liveness effect on the 1,655-run battery (per-second noise, the genuine-pump grid, aged positions): settlement keeper terms at most their SPEC / ADOPTED values, `liq_persist_n ≤ 3`, SPEC CRs as per-tier minima; the gate separates liveness failures from lender-income warnings | R4A-4 | §2.3.3 (A73) |
| 36 | Raydium adoption griefing by a standing minimum position disclosed, with the Emergency pause-opens remedy | R4B-10 | §2.6, §10 O18 |
| 37 | **M0b list completed** against the crate: `switch_liquidity`, `risk::{s1_legs, s2_legs}`, `FeeBps::capped`, `ExitParams.liq_persist_s`, the τ argument, `waterfall::settle`, plus every v0.4.1 change | R4A-6 | §9 M0b |
| 38 | Text fixes: changelog row 24 (eight §10 rows), `claim_sol` CU (§6.1 = §6.6), the S3 condition, R3A-8 (a) "resolved pending O15", the sim header's `settle_s2_first` | R4A-10 | changelog, §6.6, Appendix A |
| 39 | **Doc sync after M0b** (from the `shortwin-math` README's documentation items; no design change): one parameter-validation source — `params::validate` = `shortwin_sim.validate_params`, which now checks every ECON §15 bound of the parameters it models, and `params::validate_econ` only for the parameters it does not model (§2.3.3, §3.1, §8.1, §9 M0b); the §3.0 ring entry `(ts, p, l, x, y, lo, hi, llo, lhi)`; the reserve wording (§2.1, §8.1, §9 M2: liquidation none, expiry rent and fee); the D4 margin with an undefined EMA quote (§3.4, §4.3); §2.2 fallbacks and signatures; §2.3.6 `cal_day` never backwards; §4.5 "sustained" uptrend; §4.6 integer cap forms; §4.3 the straddle dip at `rT`; §4.5, §10 O15 as recorded (S2 and S3); §9 M0b generator and `integrate` wording; §1.1 / U18 the resolved Agave pin | M0b notes | §1.1, §2–§4, §8.1, §9, §10, §12 |

## Changelog v0.3 → v0.4 (third review round)

The round-3 brief asked for two things at once: fix every finding, and remove complexity that the reviewers showed buys no safety. Rows marked **(simplification)** delete mechanisms. Earlier changelogs are in Appendix B.

| # | Change | Finding / decision | Where |
|---|---|---|---|
| 1 | **Fixed liquidation bound** `δ_liq = delta_liq_bps` (2 %, compiled [2 %, 5 %]) **(simplification)**. The Dutch exit clock is gone: no ring `lo`, no executable-run scan, no `liq_exec_since`, no `delta_liq_min/slope/max` and no I11 clock clause. | decision 1; R3A-7; R3A-11 (c) | §3.6, §4.5, I11 |
| 2 | *(Withdrawn for `liquidate` in v0.4.1, row 25; kept for `expire`.)* **Penalty and expiry fee reserved in `max_in`** like rent: `max(V − R − π_ref − ert, B̂_ref·(1 + ε))`. | decision 1 verification (ECON #50) | §3.6, §3.7, §5.2 |
| 3 | **One settlement rule, the settlement box** **(simplification)**: arming, refutation, "not liquidatable", the S1/S2 kind and the S2/S3 amount must each come out the same on the box spanned by `(P_ema, L_ema)` and the ring medians `(P_med, L_med)`. It replaces the FV gate, the raw refutation and the kind chosen from raw legs. An armed clock keeps running on every call that is not a box-robust refutation. | decision 5; R3A-1; R3A-6; V2-9 | §3.8, §4.5 (A65) |
| 4 | Liquidation EMA leg on the **newest `liq_persist_n` ring entries**; `liq_persist_s`, `settle_persist_s` removed **(simplification)**. | decision 2 | §4.5 |
| 5 | **Ring entries cache the reprojection** `(x, y) = reprice(l, p)` at integration **(simplification)**; `lo` dropped; `RingEntry` = 104 B. `reprice` rounds so that `Y/X ≥ P`. | decision 3; R3B-7 (c); R3B-3 | §2.3.4, §4.4 |
| 6 | **Every venue change rewrites the ring's liquidity** (`l`, `lobs`, `(x, y)`; since v0.4.1 `l`, `llo`, `lhi`, `(x, y)`) and clears the bucket: graduation, Raydium adoption, fast path. | R3B-7 (a); R3A-11 (d) | §2.5, §2.6, §4.4 (A72) |
| 7 | **Event-driven venue-upgrade latch** in a new `VenueGuard` account: permissionless `flag_venue_upgraded` reads a ProgramData once; `open_short` checks a bit; `revalidate_venue` clears it **(simplification)**. The per-open 10 MiB ProgramData read, the `VenueSlots` mirror, `sync_venue_slot` and `venue_sync_max_age_slots` are gone. | decision 4; C1-15; V1-11 | §2.3.2, §3.1, §3.4, §6.5 (A7) |
| 8 | **O1/O2 knobs not built** **(simplification)**: `fv_liq`, `settle_s2_first`, `margin_ema_own_sale` are removed from `Params`, the position snapshot and `validate_params`; bytes are reserved. | decision 5; R3A-8 (c) | §2.3.3, §2.3.5 |
| 9 | **Raydium pool selection**: a non-fast adoption happens only while the market has no open position (25 % margin; 5 % inside the listing window). The pending switch and `pool_switch_delay_s` are removed **(simplification)**. The fast path runs on the explicit predicate `pool_fail` with its own chain `pool_fail_since` / `last_pool_fail_obs`. | decision 7; R3A-4; R3B-8 | §2.6, §5.3 (A42, A71) |
| 10 | **Lender SOL (option a)**: a MarketVault pot `unmatched` receives realised SOL beyond its receiver's entitlement and is drawn by `claim_sol` up to the claimant's own deficit (no division, no keeper recycling). **Lump credits vest** over `lump_vest_s` (6 h). SOL credited while `S = 0` without a live wipe record goes to `protocol_fees` (`unassigned_sol` removed). The lock state moves to `MarketVault`, so `claim_sol` runs pool_model's full touch without write-locking `Market`. | R3A-2; R3B-2; R3A-10 | §2.3.6, §3.3 (A56, A70) |
| 11 | **Withdrawal locks expire** at `lock_until = max(now, max_deadline) + lock_grace_s`; Π / `b_min` is kept. | R3B-1; decision 6 | §3.3 (A57) |
| 12 | `rent_protocol_bps` **snapshot per position** and folded into `A_j`; the `receivable` clamp is stated. | R3A-5; R3B-9 | §2.3.5, §4.5 |
| 13 | **Liquidation keeper reward ≥ `K_s`**, the top-up paid from the borrower remainder. | R3B-10 (N22) | §3.6, §5.2 (A74) |
| 14 | **Numerics**: ECON §2.1 is normative; u128 fast paths everywhere; 256-bit intermediates only for the products ECON §2.2 lists; limb division (Knuth D) only; no U256 square root; `P_ACC_MAX`, `P_OPEN_MAX`. **CU table recomputed from the math crate's primitives measured on SBF v3** [V18]. | decision 8; R3A-12; R3B-3 | §2.2, §6.1 (A68) |
| 15 | **Zero-copy layouts use `PodU128` / `PodI128`** (`[u8; 16]`, align 1) with explicit padding; compile-time size and offset assertions on sbpfv3, x86_64 and wasm32 (T-LAYOUT). `Market` recomputed: 2,848 B (v0.3 ≈ 3,050). | decision 9; R3B-4 | §2.3, §8.2 (A67) |
| 16 | **Keeper**: duties with batching, `sol_settle`/`liquidate` counted as observations, reference to ECON's cost model; **degraded mode** (the `CoverageGap` event, pause opens with reason `KeeperCoverage`, an optional pause-opens-only `coverage_authority`, owner O17). | R3B-5; decision 10 | §3.9, §3.10, §7.1 (A69) |
| 17 | **Compiled bounds tightened** and `EMA_TAU_S` made a constant; relational checks including `liq_persist_n + kappa_growth_n ≤ 7`; every governance proposal passes `sim/config_check.py` (CI gate). | R3B-6; R3A-11 (a, b) | §2.3.3 (A73) |
| 18 | `util_kink_bps` has **its own version**; a forced EMA adoption never advances it. | R3A-9 | §2.3.2, §3.0 |
| 19 | Market-level route predicates (the S3 chain, `sol_settle`'s "no route now", `pool_fail`) use the **global** `venue_fee_hard_cap_bps`; risk quotes and FV use the position's locked value. A26 says so. | R3A-4 (c) | §2.3.5, §5.3 |
| 20 | **`complete_curve` helper** (a separate stateless program) reads `rT` at execution and completes the curve for the straddle bundle. | R3B-11 | §5.4 (A66) |
| 21 | O13 (age from listing) extended to PumpSwap and pre-`T_CLAMP` CPMM pools. | R3B-12 | §4.7, §10 |
| 22 | New Emergency powers (O6) are gated **before** they are built (a SPEC §9 amendment first). | R3A-8 (b) | §9, §10 |
| 23 | **M0b "math crate v0.3 → v0.4"** inserted before M1, with the exact delete / change / add list for `crates/shortwin-math`. §8.1 and M0b point at the v0.4 sims, which now implement the program's formulas with no exceptions. | R3A-3 | §8.1, §9 |
| 24 | §10 owner decisions reduced to eight rows, each with a recommendation (v0.4.1 text fix: v0.4 said "nine"); Appendix A "Review resolution — round 3". | all | §10, Appendix A |

### Removed in v0.4

| Removed | Kind | Replaced by |
|---|---|---|
| Dutch clock: `RingEntry.lo`, `liq_exec_since`, exec-run scan, `delta_liq_min_bps` / `_slope_bps` / `_max_bps` (params and snapshot), I11 clock clause, A51 | mechanism | `delta_liq_bps` (fixed) |
| `liq_persist_s`, `settle_persist_s` | params, snapshot | entry / update counts only |
| FV gate (A53), raw refutation, raw kind choice (A52 as written) | mechanism | the settlement box (A65) |
| `fv_liq`, `settle_s2_first`, `margin_ema_own_sale` | params, snapshot | reserved bytes |
| `VenueSlots`, `sync_venue_slot`, `venue_sync_max_age_slots`, ProgramData in `open_short` (OpenTail), A54 | account, instruction, param | `VenueGuard` latch (A7) |
| `pool::recycle`, the keeper's recycling duty, `unassigned_sol` | mechanism, field | `MarketVault.unmatched`; credits with `S = 0` to the wipe record or `protocol_fees` |
| Raydium pending switch, `pool_switch_delay_s`, non-fast `execute_pool_switch` | mechanism, param | immediate adoption while `open_positions == 0` |
| `ema_tau_s` as a parameter | param | compile-time `EMA_TAU_S = 5` |
| Global `rent_protocol_bps` in rent crediting | rule | per-position snapshot folded into `A_j` |

## Conventions

- **SPEC §x**: taken from `docs/SPEC.md` v0.3.
- **D1–D4**: the owner decisions recorded in SPEC v0.3 §0 and §14 on 2026-09-27. Binding.
- **ADOPTED Qn**: a first-pass owner question whose recommended default the owner adopted (revision brief). Still a configurable parameter with compiled bounds.
- **ECON §x / ECON Gnn / ECON Nn / ECON I-xx**: taken from `docs/design/ECONOMICS.md` v0.4 (section, register item, or invariant).
- **An (PROPOSED)**: an architecture decision that is not in SPEC, introduced or made concrete here. Every An is listed in §11. **OWNER** marks the few items that still need owner confirmation; they are collected in §10.
- **UNVERIFIED**: could not be confirmed against a primary source. Each one has a test gate (§12).
- Citations are in brackets and resolved in §13. `[V1]`–`[V20]` are checks run for this document on 2026-09-26/27.
- Amounts: lamports (u64), raw token units (u64), bps (1 bp = 0.01 %), ppm for venue fee rates (ECON §2.1). `WAD` = 10¹⁸. `PodU128` / `PodI128` are 16-byte little-endian wrappers with alignment 1 (§2.3).

---

## 0. Summary

**What gets built (MVP).** One Anchor 1.2.0 program (`shortwin`), a `no_std` math crate and a `no_std` venue-state crate that the program, the Rust keeper and the WASM frontend all share, plus a tiny stateless helper program (`complete-curve`) used only by straddle bundles. Venues (D1, D2; SPEC §3):

| Venue kind | Used for | Adapter (CPI) | SPEC tier |
|---|---|---|---|
| `PumpCurve` | pump.fun coins on the live bonding curve | `sell_v2` / `buy_v2` | D |
| `PumpMigrating` | completed curve, canonical pool not yet created | none: exits pause, S3 after the migration pause | D |
| `PumpSwap` | pump.fun graduates on their canonical pool (D1) | `sell` / `buy` (exact base out) | A / B / C |
| `RaydiumV4` | tokens whose adopted SOL pool is Raydium AMM v4 (D2) | `swap_base_in_v2` / `swap_base_out_v2` (tags 16 / 17) | A / B / C |
| `RaydiumCpmm` | tokens whose adopted SOL pool is Raydium CPMM (D2) | `swap_base_input` / `swap_base_output` | A / B / C |
| `None` (list-only) | no eligible constant-product SOL pool: tokens only on Orca Whirlpools, Meteora DLMM or Raydium CLMM, tokens with no pool, ineligible pools | none: deposits and withdrawals only | — |

Jupiter is a v1.1 price-improvement path, bounded by the on-chain pool quote (D1, ADOPTED Q20).

**Key structural choices.**

1. **One external signer per market.** A system-owned, zero-data `trader` PDA is the only key the program ever signs with towards a venue. The program never forwards a user's signature to a venue; the straddle's curve completion therefore lives in a separate helper program (§5.4).
   - Lender tokens sit in a PDA-addressed `pool_vault` whose authority is the `market` PDA. Position SOL sits on the `ShortPosition` account itself. Lender SOL, the lender pot and protocol fees sit on `MarketVault`.
   - Anyone can donate to the trader or create accounts for it. Every venue leg therefore **normalises** trader state first and measures against the post-normalisation baseline (§2.4).
2. **Pricing uses exact on-chain venue math.** All four venues are constant-product pools. Their quotes are reproduced to the raw unit against mainnet trades [R-PUMP §4.1; R-VEN §2.4; R-RAY §3.4]. CR, caps, tiers, S1/S2 and fair value use that math on live reserves and on EMA reserves. A keeper or frontend quote is never trusted.
3. **Manipulation resistance comes from the observation layer, not from the venue** (ECON §5): per-second observation buckets with directional aggregation, a one-second lag, an 8-entry EMA ring whose entries cache their reprojected reserves, liquidation persistence on the newest ring entries, a fixed 2 % bound over a manipulation-resistant reference on every keeper-initiated buyback — a liquidation reserves nothing below it, so it never waits while the buyback fits (ECON §8.2) — and **one rule for every settlement decision**: the decision must come out the same on the box spanned by the EMA state and the medians of the ring's bucket lows and highs, which only 5 attacker-only seconds of 8 can move (v0.4.1; ECON §5.6).
4. **Venue state machine per market:** `None ⇄ {PumpCurve → PumpMigrating → PumpSwap}` for pump coins, `None ⇄ {RaydiumV4 ⇄ RaydiumCpmm}` for Raydium tokens. Pump pools are derived from the mint; Raydium pools are adopted by a permissionless rule that changes a pool with open positions only when it fails (§2.6).
5. **Rent is O(1)** with three per-market basis indices; each position's rate curve and protocol share are locked at open (§4.5).
6. **Lender SOL is live and bounded** (ECON §9.4): continuous entitlement, claims from each lender's realised allocation plus a pot capped at its own deficit, lumps (and the premium of an S2/S3 take over market value) vested over 6 h with a value-weighted end, locks that expire with the positions they stand for — S3 tail included — and stop counting for every lender at that moment, through a day-bucket calendar (v0.4.1).
7. **Admin powers**: timelocked parameters inside compiled bounds, proposals gated by the ECON liveness battery; a no-timelock Emergency multisig that can pause opens, disable a venue for opens, flag a route as broken (S3 evidence only) and publish bounded trailing-account templates for venue CPIs (both gated by owner item O6). No instruction lets any admin move pool or position funds, and venue program IDs cannot be changed without a program upgrade (SPEC §9–§11; C0-8).
8. **Venue guard (ADOPTED Q13).** New opens are refused once a venue program the market uses has been upgraded and not yet re-validated. A permissionless latch records the upgrade (§3.1). Exits never check it.
9. **Keepers are an operating cost with a visible failure mode.** The in-house keeper observes near thresholds every second, batches cranks, and when it cannot cover a market that market's opens pause and the program emits `CoverageGap` (§7.1; ECON §5.8).

**Verified today.** Venue deploy slots are unchanged at slot 450,944,934 (re-checked for v0.4.1), including Raydium AMM v4 (445,763,204) and CPMM (445,763,504) [V9]. The per-writable-account block limit of 24 M CU is an unconditional constant in Agave master [V10]. In the pinned pump IDLs, `claim_cashback` needs no signature from the user, and PumpSwap puts no ATA constraint on the user's token accounts [V11]. Token-2022, like p-token, clears a delegate when its allowance reaches zero [V12]. The math crate's primitives were measured on SBF v3 with platform-tools v1.57 (for example a u128 `mul_div` costs ≈ 690 CU and its 256-bit fallback ≈ 780 CU) [V18]; Agave charges 946 CU per CPI and 1,500 CU per PDA derivation [V19]; `align_of::<u128>()` is 8 on sbpfv3 and 16 on x86_64 and wasm32 [V20].

---

## 1. Monorepo layout and pinned versions

### 1.1 Pins (as of 2026-09-27)

Sources: [R-TOOL §0, §4.5, Review corrections]. Releases re-checked with `gh release list` [V8].

| Layer | Pin | Notes |
|---|---|---|
| Anchor CLI and crates (`anchor-lang`, `anchor-spl`, `anchor-client`) | **1.2.0** (2026-09-04) | Canonical repo `github.com/otter-sec/anchor`. Do not use Anchor 2.0.0-rc (alpha, unaudited) [R-TOOL §1.1] |
| Agave / Solana CLI | **4.1.2** | Anchor 1.2.0's pin: `docker/Makefile@v1.2.0` sets `SOLANA_CLI=v4.1.2`; the "recommended Solana version" sentence was added on master in `otter-sec/anchor@78ed411a40` (2026-09-14), not at the v1.2.0 tag (C1-11) [R-TOOL Review corrections 2]. `Anchor.toml [toolchain] solana_version = "4.1.2"` |
| Platform-tools / SBPF arch | **v1.57** (rustc 1.95) / **v3** | Always `anchor build --tools-version v1.57 --arch v3`, never a bare `cargo build-sbf` (defaults to v1.54 / arch v0) [R-TOOL §1.2] |
| Host Rust | **1.98.1** (`rust-toolchain.toml`) | On-chain crates declare `rust-version = "1.89"` and must build with rustc 1.95 |
| `[workspace.metadata.cli]` | `solana = "4.1.2"` | So solana-verify picks the right image [R-TOOL §5.2] |
| Solana SDK line in program | `solana-*` 3.x through anchor-lang 1.2; `spl-token-2022-interface` 2.1.x through anchor-spl 1.2 | Interface 2.x does not know Token-2022 extension 28 and fails closed (desired) [R-MINT §3.3] |
| Tests | LiteSVM **0.16.0** (+ litesvm-token 0.16.0; Agave `^4.2.1`), Mollusk **0.15.1** (CU bench; declares Agave `^4.2.0`, **not** 4.3 crates — C1-14), proptest **1.11.0**, Crucible **0.2.1** via `anchor fuzz`, Surfpool **1.6.0** | Pin the Agave versions that the on-chain workspace lockfile resolves, so LiteSVM and Mollusk share one Agave [R-TOOL Review corrections 3]. **Resolved in M0b:** resolver 3 picks Agave **4.3.0** for Mollusk's `^4.2.0`, and Mollusk 0.15.1 does not compile against it; `tests/bench/cu_math` (a standalone workspace) holds Agave at **4.2.2** in its own lockfile. The program tests' lockfile must therefore hold 4.2.x (LiteSVM's `^4.2.1` allows it) or move Mollusk. Do not use Anchor's template pin `litesvm = "0.10.0"`, `anchor-litesvm`, Trident or solana-program-test |
| Verifiable build | solana-verify **0.5.2**: `solana-verify build --library-name shortwin --arch v3 --cargo-build-sbf-args="--tools-version v1.57"` | The `=` form is required: the option is declared `require_equals(true)` (C1-5) [R-TOOL Review corrections 1]. CI asserts hash equality with `anchor build` (**UNVERIFIED** that they match) |
| Admin | Squads v4 `SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf` | Autonomous multisigs [R-TOOL §5.3] |
| JS runtime | Node **24.21.0 LTS**, pnpm **12.x** | Pin in `mise.toml` and `packageManager` |
| JS Solana | `@solana/kit` / `@solana/react` **8.3.0**, `@solana/kit-plugin-wallet` **0.20.0**, `@solana/kit-plugin-rpc` **0.19.0** | Do not use web3.js v1, wallet-adapter or `@anchor-lang/core` in the app [R-TOOL §3] |
| Codegen | `codama` **1.11.0**, `@codama/nodes-from-anchor` **1.5.6**, `@codama/renderers-js` **2.5.0** (pinned devDeps) | |
| Frontend | Vite 8.3.1, React 19.3.0, TanStack Query 5.104 / Router 1.170, Vitest 5.0.2, Biome 2.5.14, Playwright 1.63 | [R-TOOL §4.5] |
| Services | tokio 1.53.1, axum 0.8.9, sqlx 0.9.0, yellowstone-grpc-client 14.0.0 | Host-only workspace |

### 1.2 Directory tree

This follows [R-TOOL §4.3]. PROPOSED additions are marked ★.

```
short.win/
├── Anchor.toml               # [toolchain] anchor_version="1.2.0", solana_version="4.1.2", package_manager="pnpm"
├── Cargo.toml / Cargo.lock   # ON-CHAIN workspace: programs/*, crates/*, tests/*; Agave versions pinned (C1-14)
├── rust-toolchain.toml       # 1.98.1
├── mise.toml  package.json  pnpm-workspace.yaml  codama.json  biome.json
├── idls/                     # vendored third-party IDLs + PROVENANCE.md (repo@commit or on-chain account, slot, sha256)
│   ├── pump.json  pump_amm.json  pump_fees.json      # pump-public-docs@81091419 (IDL commit e0687ae9)
│   ├── raydium_cp_swap.json ★                         # on-chain PMP IDL account FHuJXL7c… (written 2026-09-11) [R-RAY §1.3]
│   └── jupiter.json                                   # v1.1 only: on-chain IDL account C88XWfp2…
│   #   Raydium AMM v4 has no IDL: its layout is hand-written from raydium-amm@d26944b state.rs/instruction.rs [R-RAY §1.3]
├── programs/
│   ├── shortwin/src/
│   │   ├── lib.rs                # #[program]; thin: each ix calls instructions::<name>::handler
│   │   ├── constants.rs          # ★ compile-time venue program IDs, static PDAs, authorities, WSOL, T_CLAMP, EMA_TAU_S,
│   │   │                         #   RING_LEN, S3_OBS_GAP_S, COVERAGE_GAP_S, P_ACC_MAX, P_OPEN_MAX, compiled bounds (C0-8)
│   │   ├── state/                # config.rs market.rs position.rs lender.rs exposure.rs oracle.rs vault.rs epoch.rs★
│   │   │                         #   venue_guard.rs★ pod.rs★ (PodU128/PodI128) layout_asserts.rs★ (T-LAYOUT)
│   │   ├── instructions/         # one file per instruction (§3)
│   │   ├── engine/ ★             # prologue.rs observe.rs settle.rs bounds.rs route.rs waterfall.rs caps.rs tier.rs
│   │   │                         #   guard.rs trader.rs (normalisation, ephemeral WSOL) pool_select.rs lender.rs
│   │   ├── venues/               # mod.rs (VenueKind, strict parsers, extras), pump_curve.rs, pump_amm.rs,
│   │   │                         #   raydium_v4.rs★, raydium_cpmm.rs★, jupiter.rs (v1.1)
│   │   ├── mint_filter.rs        # allowlist filter [R-MINT §9.2]
│   │   ├── lamports.rs ★         # direct lamport moves between program-owned accounts (checked)
│   │   ├── events.rs errors.rs
│   └── complete-curve/ ★         # stateless helper: complete_curve(max_sol_cost) for straddle bundles (A66, §5.4)
├── crates/
│   ├── shortwin-math/        # #![no_std], ZERO deps: U256 (Knuth D), constant-product quotes (fee-on-SOL-leg and
│   │                         #   fee-on-input families), B̂_str, observation/EMA/ring with cached reprojection, settlement
│   │                         #   box, exit bounds, rent basis indices, CR, caps, tiers, waterfalls, lender pool (epochs,
│   │                         #   pro-rata locks with lock_until, realised claims, pot, lump vesting) — = sims v0.4
│   ├── shortwin-venue-state/ # #![no_std]: length-tolerant readers for pump BondingCurve/Global, pump-fees FeeConfig,
│   │                         #   PumpSwap Pool/GlobalConfig, Raydium AmmInfo★, CPMM PoolState★/AmmConfig★,
│   │                         #   SPL/T22 Mint/Account, ProgramData, Pyth PriceUpdateV2; PDA helpers (host only)
│   └── shortwin-math-wasm/   # wasm-bindgen facade → packages/math-wasm
├── tests/
│   ├── litesvm/              # program integration tests + scenario builders + fixture loader
│   ├── mock_cpamm/ ★         # TEST-ONLY constant-product venue program (never deployed); used by M2 (§9)
│   ├── bench/                # mollusk-svm-bencher CU benches → bench/*.md (committed, diffed in CI);
│   │                         #   bench/cu_math ★: per-primitive math budgets (T-CU-MATH, M0b)
│   └── fixtures/{programs,accounts,txs}/ + manifest.json   # dumped mainnet ELFs/accounts/trades with provenance
├── fuzz/shortwin/            # anchor fuzz (Crucible) harness + §8.3 invariants
├── services/                 # HOST workspace (own Cargo.lock): common/, keeper/, indexer/ (+ api inside indexer)
├── clients/js/               # Codama-generated @shortwin/client (committed, CI diff-checked)
├── packages/{sdk,math-wasm,config}/
├── apps/web/
├── scripts/                  # dump-fixtures.ts, check-venue-upgrades.ts, build-alt.ts ★, find-raydium-pools.ts ★,
│                             #   config-gate.sh ★ (runs docs/design/sim/config_check.py on a governance proposal, A73)
└── docs/{SPEC.md, research/, design/}
```

Keep the two Cargo workspaces separate. Host-only crates (Agave 4.3 clients with MSRV 1.97.1, sqlx 0.9) must not enter the SBF lockfile [R-TOOL §4.2].

---

## 2. Program design

### 2.1 Module structure and responsibilities

| Module | Responsibility | Depends on |
|---|---|---|
| `state/*` | Account structs (§2.3), sizes, PDA seeds, `PodU128`/`PodI128`, compile-time layout assertions (A67) | — |
| `constants.rs` | Compile-time venue program IDs, static venue PDAs and authorities, WSOL, `T_CLAMP`, `EMA_TAU_S`, `RING_LEN`, `S3_OBS_GAP_S`, `COVERAGE_GAP_S`, `P_ACC_MAX`, `P_OPEN_MAX`, the compiled parameter bounds of §2.3.3 (C0-8) | — |
| `engine/prologue.rs` | Shared market prologue (§3.0): params-version sync → accrue (vest release, rent) → observation roll → venue sync → observation record → route chains | math, venue-state |
| `engine/observe.rs` | Observation buckets, EMA integration, ring push with the cached reprojection, ring rewrite on venue change, `CoverageGap` (ECON §5) | math |
| `engine/settle.rs` | Settlement box, S1/S2/refutation/not-liquidatable predicates, arming state machine, amount check (§3.8; ECON §5.6) | math |
| `engine/bounds.rs` | Exit reference prices, fixed δ, `max_in` (liquidation: no reserve; expiry: rent and the expiry fee senior; v0.4.1, §3.6, §3.7) (ECON §8.2) | math |
| `engine/route.rs` | Per-venue `venue_executable`; `s3_no_route` and the S3 chain; `pool_fail` and the fast-path chain (§5.3) | venue-state |
| `engine/guard.rs` | Pause flags (incl. coverage pause), `VenueGuard` latch, venue fee caps, mint guard (entry/exit), account denylist | mint_filter |
| `engine/trader.rs` | Trader normalisation, ephemeral WSOL account lifecycle, grief-proof PDA creation (§2.4) | lamports |
| `engine/pool_select.rs` | Raydium pool eligibility, locked depth, listing / zero-position adoption / fast path (§2.6) | math, venue-state |
| `engine/lender.rs` | Lender pool: touch, epochs, pro-rata allowance and locks, pot, vesting, realised claims (ECON §9) | math |
| `engine/caps.rs`, `engine/tier.rs`, `engine/waterfall.rs` | SPEC §4 caps and tiers; applies pure `math::waterfall` results as lamport/token moves | math, lamports |
| `venues/mod.rs` | `VenueKind`, per-venue `ReadSet` / `TradeExtras` parsers (strict, all mandatory), bounded extras (§3.0), `VenueState` | venue-state |
| `venues/pump_curve.rs`, `venues/pump_amm.rs` | Pump `sell_v2`/`buy_v2`; PumpSwap `sell`/`buy` | engine/trader |
| `venues/raydium_v4.rs`, `venues/raydium_cpmm.rs` | AMM v4 tags 16/17; CPMM `swap_base_input`/`swap_base_output` | engine/trader |
| `venues/jupiter.rs` | v1.1: hardened route wrapper [R-JUP §7.2] | — |
| `mint_filter.rs` | Allowlist filter [R-MINT §9.2] | anchor-spl token_2022 interface |
| `lamports.rs` | `move_lamports(from_program_owned, to, amount)` with checked math and a rent-floor assertion on `from` | — |

The program never uses `pump-rust-client` [R-TOOL §1.4] or any Raydium SDK crate. CPIs are built by hand as `Instruction { program_id, accounts, data }` with discriminators from the vendored IDLs (pump, PumpSwap, CPMM) or the one-byte tags of AMM v4 [R-PUMP §3; R-VEN §2.1; R-RAY §3.1–3.2]. `declare_program!` may generate discriminator constants, but its account types are never used to decode state (A31).

### 2.2 Numeric conventions (shortwin-math)

**Normative: ECON §2.1** (the numerics table: type, scale, rounding and order of operations for every quantity the sims compute) and ECON §2.2 (where 256-bit intermediates are needed). Rounding always favours the pool and lenders: quotes, fees, rent, penalties, margin, FV and locks up; shares, payouts, credits and claims down. This section adds only the implementation rules (A30 as amended by A68, R3B-3, decision 8):

- **`mul_div` takes the u128 path whenever `a.checked_mul(b)` succeeds**; only then does it build a 256-bit product and divide by limbs (Knuth's Algorithm D on 64-bit digits). A bit-by-bit 256-bit divider is forbidden. `shortwin-math` already does both [V18].
- **256-bit intermediates only where ECON §2.2 says a product can exceed 2¹²⁸**: the rent products `Q·coef·ΔJ`, `A_j·δJ_j`, `Q·P_ema·r·T_min`; share and accumulator products `a·(S + VS)`, `s·(T + B + VA)`, `shares·Δacc`, `lock·Π`; and, as a fallback only, σ's `Q·P` and the κ/g products at prices above ≈ 2·10³⁴, σ's numerator `n·WAD·10⁴` above `n ≈ 3.4·10¹⁶` lamports, the impact caps' `X²·(10⁴ + ι)` above `X ≈ 1.8·10¹⁷` (their square root stays u128) and the tier threshold's comparison (§4.7). Quotes (`Q·Y < 1.1·10³⁷`), `reprice`, fees, CR, the fee-scaled caps, FV, the EMA and the box check fit u128.
- **Square roots are u128 only.** `L = isqrt(X·Y)` with `X·Y < 2¹²⁸` (ECON §2.2 bounds); `reprice` uses the ceiling root `isqrt⁺(P)` with `X = ⌊L·10⁹/sp⌋`, `Y = ⌈L·sp/10⁹⌉`, so `Y/X ≥ P` and every reprojected quote is an upper bound (R3B-7 c). There is no U256 square root on-chain.
- **Division is what costs CU on SBF, not width.** Measured on sbpfv3 [V18]: a u128 `mul_div` ≈ 690 CU, its Knuth-D 256-bit fallback ≈ 780 CU, the crate's `mul_div_u64` ≈ 270 CU, a native u64 multiply-and-divide ≈ 75 CU. M0b may add a bit-identical u64 fast path (taken when the product fits u64) for fee components and splits; it changes no result.
- **Ring medians** (v0.4.1): the **lower** median of the ring's 8 bucket lows `lo` (`llo`) — the 4th smallest, index `⌊(n − 1)/2⌋` of the sorted values — and the **upper** median of the 8 bucket highs `hi` (`lhi`), index `⌊n/2⌋` (ECON §2.1).
- **Price clamps** (ECON §2.2): rent accrual uses `P̃ = min(P_ema, P_ACC_MAX)` with `P_ACC_MAX = 10²⁹`, which keeps the `J` indices below 2¹²⁸ for 100 years; opens are refused while `P_ema > P_OPEN_MAX = 10²⁷`.
- **Order of operations inside one instruction** (ECON §2.1; §3.0): rent accrues on `P_ema` **before** this instruction's roll; the completed bucket is then integrated; the instruction's own pre-trade observation is recorded; the body reads the **post-roll** EMA state, including the D4 margin's EMA leg, which quotes on the newest ring entry's cached `(x, y)` (R3A-12).
- Every function is `pub fn … -> Result<T, MathError>` (quotes: `Option`) with checked arithmetic. `#![deny(clippy::arithmetic_side_effects, clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::unwrap_used)]` [R-TOOL §5.1].
- **Math types are not account layouts.** The crate's structs (`Ema`, `Ring`, `RingEntry`, `Bucket`, `Pool`, `Lender`) hold native `u128`/`i128`. The program converts to and from its zero-copy layouts (`PodU128`, §2.3) at load and store and never casts account bytes to a math type (R3B-4).
- **Normative references** (R3A-3): the ECON v0.4.1 formulas; `sim/pool_model.py` v0.4.1 for lender accounting; `sim/shortwin_sim.py` v0.4.1 for quotes, observation, exit bounds, settlement and waterfalls; `shortwin_sim.validate_params()` for the parameter checks. v0.4 removed every known deviation of the sims from the program's formulas (ECON "Sim deviations of v0.3, all fixed") and v0.4.1 the last one found (the S2/S3 lump, R4A-5), so the differential suites (§8.1) run **without tolerance or exception**. The sims' analysis-only switches (`box_obs`, `pprime_raw`, `ref_cap_lo = False`, `liq_reserve ≠ "none"`, …) exist only for comparison rows; vectors are generated with the defaults.

Public API (names are binding for the keeper and WASM; v0.4 changes marked ★, v0.4.1 ★★):

```
// quotes: all return None when the quote is undefined (q ≥ token reserve, complete curve, …)
quote::sol_leg::{buy_exact_out(q, X, Y, eps1, &FeeBps), sell_exact_in(q, X, Y, &FeeBps, real_quote_cap)}  // pump curve / PumpSwap
quote::ray_v4::{buy_exact_out(q, X_sol, Y_tok, num, den), sell_exact_in(a, X_tok, Y_sol, num, den)}      // R-RAY §3.3
quote::cpmm::{buy_exact_out(q, X_sol, Y_tok, &CpmmFees), sell_exact_in(a, X_tok, Y_sol, &CpmmFees)}      // R-RAY §3.3
quote::curve_straddle(q, &CurveState, &FeeBps, &MigrationParams, &PammFeeSchedule) -> Option<BuyQuote> // B̂_str (V1-9)
quote::curve_two_leg(..) -> Option<BuyQuote>                           // B̂₂: v1.1 in-program straddle only; never called by the MVP
quote::reprice(L, P) -> Option<(X, Y)> ★isqrt⁺, X floor, Y ceil ; quote::curve_reprice(L, P, offsets) -> Option<CurveState>
fees::{pump_curve(..), pump_amm(..), ray_v4(pool), cpmm(config, pool, direction)} -> the fee components; oneway_ppm() = φ in ppm ★ (ECON §2.1)
observe::{record, integrate ★caches (x, y), ★★stores the bucket's (lo, hi, llo, lhi), reports the gap, robust_spot, sustained_growth,
          ring_medians ★★(lower of lo/llo, upper of hi/lhi), rewrite_liquidity ★(ring l, ★★llo, lhi, x, y; clears the bucket)}
persist::liq_ema_leg(&Ring, below, n) -> bool ★                         // newest n entries on the cached (x, y); no seconds
settle::{box_of(&Ema, &Ring, now) ★, s1_legs ★, s2_legs ★, refuted ★, not_liquidatable ★, amount ★(p' per corner ★★),
          evaluate(&mut ArmState, ..) ★}
bounds::{liq_reference(..) -> P_ref ★★(also ≤ lo_x·(1 + κ_eff)), expiry_reference(..) -> P_ref,
         max_in_liq(B_ref, δ, V, K_s, ert) ★★, max_in_exp(B_ref, δ, ε, V, K_s, R, π_ref, ert) ★★}
rent::{accrue(&mut Basis, P_ema ★★(P̃ = min(P_ema, P_ACC_MAX) inside), U, k, dt), due, credited_for(&Pos ★incl. p_pos, &Basis), lender_credit(&Aggr ★folded, ΔBasis), min_rent}
risk::{cr_bps, cr_below, spot_leg, sigma, sigma_xy, margin_d4, fair_value, k_s, k_e, liquidation_price}
caps::{max_q_position, max_total_q, fee_size_cap, fee_oi_cap, util_ok, curve_gamma_ok, wallet_ok, conservative_x0}
tier::assign(&TierInputs) -> Tier
pool::{deposit ★★wipe-sweep gate, withdraw ★lock_until ★★calendar, allowance, claim ★pot, cover, touch ★to_pot, sweep ★vests,
       release ★, vest ★★value-weighted end, cal_advance ★★, borrow ★(now, q, ★★release_at), repay ★(Resolution{q, credited,
       lump}, rent_paid, p), settle ★(Resolution, sol_in), maybe_roll ★flushes vest, resolve}      // = sim/pool_model.py v0.4.1
       // every time-dependent function takes now; withdraw takes lock_grace_s, vest / sweep / repay / settle lump_vest_s;
       // deposit takes rewards_required, sweep a SweepSource; p (rent_protocol_bps) enters at repay (crate README)
pool_select::{locked_depth ★λ in bps, eligible(..), dominates(..), fast_path_qualifies(..)}                      // §2.6
waterfall::{close, liquidate ★keeper floor, expire, settle ★★(v, kind, k_s, fv, q, p_ema, r_due) returns the vesting lump} -> Payout  // Σ legs == V
```

Removed from the v0.3 API (M0b): `persist::exec_run`, `persist::ema_run` as a Dutch-clock helper, `bounds::dutch_delta`, `risk::fv_ok`, `risk::fv_gate`, `risk::refuted` (raw), `risk::settle_kind` (raw), `risk::{s1_legs, s2_legs}` (single-point forms; the box forms live in `settle`), `observe::switch_liquidity` (replaced by `rewrite_liquidity`), `observe::obs_median`, `bounds::max_in` (split into `max_in_liq` / `max_in_exp`), `pool::recycle`, `pool::touch_readonly`, the `unassigned_sol` field.

### 2.3 Accounts

All PDAs are under the short.win program id. Bumps are stored and re-used with `create_program_address`; `find_program_address` is never called on hot paths.

**Zero-copy layout rule (A67; R3B-4).** `align_of::<u128>()` is 8 on sbpfv3 but 16 on x86_64 (rustc ≥ 1.77) and on wasm32 [V20], so a `#[repr(C)]` struct with raw 128-bit fields has different padding, size and offsets on the program, the keeper and the WASM frontend (v0.3's 88-byte `RingEntry` is 96 bytes on the host). Every zero-copy account (`GlobalConfig`, `VenueGuard`, `VenueExtras`, `Market`, and since v0.4.1 `MarketVault`) and every type nested in one therefore:

- stores 128-bit values as `PodU128` / `PodI128`: `#[repr(transparent)]` newtypes over `[u8; 16]` (little-endian, alignment 1, `bytemuck::{Pod, Zeroable}` through Anchor's re-export, `get()` / `set()`), declared in `state/pod.rs` so that the IDL — and hence Codama and `declare_program!` — describes them as 16-byte arrays; no raw `u128`/`i128` field is allowed (CI lint over `state/`);
- has every field at an offset that is a multiple of its alignment (≤ 8) **with all padding written out** as `_padN: [u8; k]`, and a size that is a multiple of 8;
- carries compile-time assertions of `size_of` and of every field's `offset_of!` in `state/layout_asserts.rs`, compiled for sbpfv3 (`anchor build`), x86_64 (`cargo test`) and wasm32 (the WASM build). A difference fails the build on the target that differs (T-LAYOUT).

Borsh accounts (`ShortPosition`, `LenderPosition`, `ShareEpoch`, `WalletExposure`, `OracleCache`) serialise 128-bit values as 16 little-endian bytes with no alignment, so the rule does not apply to them.

#### 2.3.1 Overview

Sizes include the 8-byte discriminator. Rent-exempt minimum = `(len + 128)·5,080` lamports [V5].

| Account | Kind | Seeds | Size (bytes) | Created by | Mutated by |
|---|---|---|---|---|---|
| `GlobalConfig` | zero-copy | `["config"]` | ≈ 1,450 (exact in T-LAYOUT) | `initialize_config` (program upgrade authority, once) | `apply_config`, `set_pause(global)`, `disable_venue`, `set_route_broken(venue)` — admin instructions only, never a hot path |
| `VenueGuard` ★ | zero-copy | `["venue_guard"]` | 224 | `initialize_config` | `flag_venue_upgraded` (anyone; writes only when a bit or the recorded slot changes) and `revalidate_venue(venue, expected_slot)` (Emergency); read only by `open_short` (A7) |
| `VenueExtras` | zero-copy | `["venue_extras", venue u8]` | ≈ 3,500 | `initialize_config` (empty sets; unused until O6) | `set_venue_extras` (Emergency or Governance; gated by O6) only; read by trading instructions of that venue (A41, V2-6) |
| `OracleCache` | Borsh | `["sol_usd"]` | 45 | `initialize_config` | `crank_sol_usd` (anyone) |
| `Market` | zero-copy | `["market", mint]` | **3,104** (v0.4.1: 136-B ring entries) | `create_market` (anyone) | Every trading instruction, `crank_price`, `sol_settle`, `deposit`, `withdraw`, pool-selection instructions; `set_pause(market)`, `set_coverage_pause`, `set_route_broken(market)`, `flag_unsafe_mint`. **Not** `claim_sol`, `cover_deficit`, `sync_lender`, `add_margin` |
| `MarketVault` | **zero-copy** (v0.4.1) | `["market_vault", market]` | **2,200** | `create_market` | Every instruction that moves lender SOL, touches a lender or resolves inventory: exits, settlements, `open_short` (open fee, borrow), lender instructions (pot, lock state), trader sweeps, reward claims, `sweep_protocol_fees`. Holds `lender_sol`, the pot `unmatched`, `protocol_fees`, the lock state and the lock expiry calendar (§2.3.6) |
| pool token vault | SPL/T22 token account | `["pool_vault", market]`, authority = `market` | 165/170 | `create_market` | `deposit` (in), `withdraw` (out), open (out), exits (in). A delegate exists only inside a Raydium sell leg (§2.4) |
| `trader` | **system-owned, zero data**, 0 lamports at the end of every short.win instruction | `["trader", market]` | 0 | never allocated | Venue legs; third parties may credit it between instructions (normalised, §2.4) |
| trader base ATA (pump venues) | token account | `ATA(trader, mint, market.token_program)` | 165/170 | `create_market` (pump venues) or the PumpSwap transition | Pump venue legs only; balance 0 at the end of every short.win instruction |
| `trader_wsol` | ephemeral SPL-Token WSOL account at a short.win PDA address, owner (authority) = `trader` | `["trader_wsol", market]` | 165 | Inside each PumpSwap / Raydium leg (grief-proof creation, §2.4) | Closed in the same instruction |
| `ShortPosition` | Borsh | `["position", market, borrower, nonce u64 LE]` | **384** | `open_short` (payer borrower) | `close_short`, `add_margin`, `liquidate`, `expire`, `sol_settle` (arming or settling) |
| `LenderPosition` | Borsh | `["lender", market, owner]` | **241** | `deposit` (init_if_needed, payer owner) | `deposit`, `withdraw`, `claim_sol`, `cover_deficit`, `sync_lender` (anyone may call `sync_lender`: epoch conversion, clearing the lender's own expired lock fields, surplus to the pot, §3.3; the aggregate no longer depends on it, v0.4.1) |
| `ShareEpoch` | Borsh | `["share_epoch", market, epoch u32 LE]` | **127** | `sol_settle` when it rolls the epoch (payer keeper, grief-proof creation) | Immutable, except that the record of a **full wipe** is topped up by SOL credited while the market has no shares, until the next deposit finalises it (V2-12) — which waits until the trader sweep and the venue-reward claim have credited the record (v0.4.1, R4A-7) |
| `WalletExposure` | Borsh | `["exposure", market, borrower]` | 81 | `open_short` (init_if_needed) | open (+Q), exits (−Q); closed to the borrower at 0 |
| `swap_auth` (v1.1) | system-owned, zero data | `["swap_auth", position]` | 0 | never allocated | Jupiter adapter only [R-JUP §4.3] |
| pump UVA of trader | pump-owned | pump `["user_volume_accumulator", trader]` | 137 | `create_market` for pump-curve markets (CPI `init_user_volume_accumulator`, payer = creator) | pump |
| PumpSwap UVA of trader | pAMM-owned | pAMM `["user_volume_accumulator", trader]` | 137 | `create_market` if already graduated; otherwise the keeper at graduation (top-level `init_user_volume_accumulator`, `payer` = keeper, `user` = trader) | pAMM |

Raydium markets need no trader ATA, no UVA and no venue-side accounts at all [R-RAY §4.1].

**A4: vault topology.** SPEC §10 names a "pool vault (tokens)", a "pool SOL vault" and "vault SOL" per position:

- **Position SOL = extra lamports on the `ShortPosition` account.** Invariant `lamports ≥ rent_min(len) + position.v`. The program owns the account and debits it directly, with no CPI. It is never passed to an external program.
- **Lender SOL + protocol fees = `MarketVault` lamports.** Invariant `lamports ≥ rent_min + vault.lender_sol + vault.protocol_fees`. The pot `unmatched` and the unreleased vest `vest_bal` are parts of `lender_sol`, not extra lamports. Protocol fees accrue per market and are swept by a permissionless instruction (A9).
- **Pool tokens = PDA-addressed token account** (`["pool_vault", market]`), not an ATA: a pre-created ATA would permanently block `init` [R-MINT §8.2].

#### 2.3.2 `GlobalConfig`, `VenueGuard`, `VenueExtras` (zero-copy)

`GlobalConfig`:

| Field | Type | Notes |
|---|---|---|
| `version` | u16 | Layout version |
| `bump` | u8 | |
| `flags` | u8 | bit0 `paused_opens_global` |
| `venue_enabled` | u32 bitmask | bit0 pump curve, bit1 PumpSwap, bit2 Raydium AMM v4, bit3 Raydium CPMM, bit4 Jupiter (v1.1). Enabling is timelocked; disabling is immediate (`disable_venue`) and affects opens only |
| `venue_route_broken` | u32 bitmask | Same bits. Set/cleared by Emergency (`set_route_broken`, O6). Refuses opens and is S3 evidence (`s3_no_route`, §5.3). Never an input to a quote, a leg, an exit or `pool_fail` (A40, V2-1, V1-7) |
| `timelock_s` | u32 | Default 172,800. Cannot go below the compiled `MIN_TIMELOCK_S = 86,400` (A8) |
| `governance` | Pubkey | Squads Governance vault (48 h Squads time_lock) [R-TOOL §5.3] |
| `pause_authority` | Pubkey | Squads Emergency vault (time_lock 0) |
| `coverage_authority` ★ | Pubkey | Keeper-held key that can only set/clear a market's `paused_coverage` flag (degraded mode, A69). `Pubkey::default()` = disabled, the default until the owner decides O17; set through a timelocked `Authorities` change |
| `fee_recipient` | Pubkey | Destination of `sweep_protocol_fees` |
| `pyth_sol_usd` | Pubkey + `[u8;32]` feed id | `7UVimffx…`, feed `ef0d8b6f…` [R-VEN §5] (the receiver program id is a constant) |
| `params` | `Params` | §2.3.3; version number `params_version` u32 |
| `tiers` | `[TierParams;4]` | A, B, C, D |
| `ema_params` | `{ema_dtmax_s u16, _pad u16, ema_clamp_bps u32, liq_slow_tau_s u32, liq_slow_dtmax_s u32, version u32, _pad u32, force_at i64}` | Market-level; a market adopts a new `version` while it has no open position, or at its first prologue after `force_at = applied_at + ema_force_delay_s` (A17, A59; V2-13). τ is the constant `EMA_TAU_S` (R3B-6) |
| `kink` ★ | `{util_kink_bps u32, version u32}` | Market-level with **its own version**, adopted only while the market has no open position; a forced EMA adoption never advances it (R3A-9) |
| `pending` | `PendingChange` | `{kind u8 (0 = none), _pad [u8;7], not_before i64, payload ConfigChange}` |
| `_reserved` | `[u8;256]` | |

Venue program IDs, their static PDAs (pump `global` `4wTV1Ymi…`, `event_authority` `Ce6TQqeH…`, GVA `Hq2wp8uJ…`, fee config `8Wf5TiAh…`; pAMM `global_config` `ADyA8hde…`, `event_authority` `GS4CU59F…`, GVA `C2aFPdEN…`, fee config `5PHirr8j…`; AMM v4 authority `5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1`; CPMM authority `GpMZbSM2GgvTKHJirzeGfMFoaZ8UR2X7F4v8vHTvxFbL`; the ProgramData addresses) and the Pyth receiver id are **compile-time constants** in `constants.rs`, verified by a unit test that re-derives each PDA. No config path can change them; only an audited program upgrade can (C0-8) [R-PUMP §3.1; R-VEN §1.1; R-RAY §1.1].

Only governance writes after a timelock; Emergency writes the flags (and `VenueExtras`, `VenueGuard` through `revalidate_venue`). `GlobalConfig` is **never written on a hot path or by a permissionless instruction**, and it is read (zero-copy `load()`) by every instruction.

**`VenueGuard` (A7; decision 4; hardened in v0.4.1, R4B-7).** `version` u16, `bump` u8, `_pad0` u8, `upgraded` u32 (one bit per venue program: pump, pAMM, pfee, AMM v4, CPMM, Jupiter), `guard_slot [u64;6]` (last validated ProgramData deploy slot; today 449,734,335 / 446,462,733 / 446,465,969 / 445,763,204 / 445,763,504 / 449,280,566 [V9]), `flagged_slot [u64;6]` (the latest deploy slot seen while flagged), `flagged_at [i64;6]`, `_reserved [u8;64]` = 224 B. The permissionless `flag_venue_upgraded` sets a bit when a ProgramData deploy slot differs from `guard_slot`, and — while the bit is set — refreshes `flagged_slot` / `flagged_at` whenever the deploy slot changes again (a second upgrade during review is visible). Emergency `revalidate_venue(venue, expected_slot)` requires the ProgramData's current deploy slot to equal `expected_slot`, the slot its reviewers validated, then records it and clears the bit, so an upgrade that lands between review and execution cannot be validated silently. `open_short` reads the account and refuses while a bit of its venue's **mask** is set: `PumpCurve` = pump + pfee + **pAMM** (a curve position exits on PumpSwap after graduation), `PumpSwap` = pAMM + pfee, `RaydiumV4` and `RaydiumCpmm` = **both** Raydium programs (the fast path can move a market between them). It is kept out of `GlobalConfig` so that a permissionless writer never write-locks the account every instruction reads (the lesson of V1-11).

**`VenueExtras` (A41, V2-6; gated by O6).** Per venue: for each instruction kind (sell, buy, claim) two sets — `current` and `previous` — of up to 4 entries, plus a `version` per set. An entry is either a fixed key or a **derivation template** `{program: Pubkey, seeds: [SeedPart; ≤ 4], writable: bool}` where a `SeedPart` is constant bytes (≤ 32) or a placeholder among `{mint, pool, trader, bonding_curve, coin_creator}`; the caller passes each derived account plus its bump, and the parser re-derives it with `create_program_address` (no search). `set_venue_extras` moves `current` to `previous` and writes the new set; callers choose `none`, `current` or `previous` in the instruction data, so one hostile or wrong Emergency write can never brick exits.

#### 2.3.3 Parameters (`Params`, `TierParams`)

Names, defaults and hard bounds are **identical to ECON §15**. Scope: G global; T per tier; P snapshot into each position at open; M market-level (a market adopts a new value while it has no open position; the EMA fields also after `ema_force_delay_s`, A59; the kink has its own version); C compile-time constant. `validate_params()` rejects any value outside its hard bounds and checks the relational bounds below the table (A8). **One source of truth:** on every parameter `shortwin_sim` models it is the same function as `shortwin_sim.validate_params()` — `shortwin_math::params::validate`, which accepts exactly what the sim accepts (T-CONFIG), including the penalty split's parts and every per-tier bound — and the bounds of the parameters the sim does not model (`min_notional`, `market_max_notional`, `market_creation_fee`, `boost_dust_lamports`, `boost_window_s`, `market_warmup_s`, `ema_force_delay_s`, `lump_vest_s`, `lock_grace_s`, `theta_A` / `theta_B`, the four oracle limits, `pool_min_real_sol`, `pool_switch_margin_bps`, `pool_list_window_s`, `pool_list_margin_bps`, `pool_fast_min_bps`) are `params::validate_econ`, transcribed from ECON §15; the program runs both (§3.1). Every `Params` / `TierParams` / `EmaParams` proposal must also pass `sim/config_check.py` (the ECON §5.7 liveness battery, 1,655 runs since v0.4.1) before governance signs it (A73; R3B-6, R4A-4). **Bold** = changed in v0.4 or v0.4.1.

| Parameter | Default | Hard bounds | Unit | Scope | Source |
|---|---|---|---|---|---|
| `open_fee_bps` | 25 | [0, 100] | bps of N_open | G | SPEC |
| `rent_protocol_bps` | 1,500 | [0, 3,000] | bps of rent | **G→P** | SPEC; snapshot per position and folded into `A_j` (R3A-5, R3B-9) |
| `penalty_split` lender / protocol / keeper | 5,000 / 2,000 / 3,000 | keeper ≤ 5,000, protocol ≤ 5,000, sum 10,000 | bps | G→P | SPEC |
| `expiry_fee_bps` | 200 | [0, 500] | bps of base | G→P | SPEC |
| `settle_keeper_fixed` / `_bps` / `_cap` | 10,000,000 / 100 / 250,000,000 | **[0, 10,000,000] / [0, 100] / [0, 250,000,000]** | lamports / bps of V / lamports | G→P | SPEC / SPEC / ADOPTED Q4 (v0.4.1: larger values widen the S1 region, R4A-4) |
| `expiry_keeper_fixed` / `_bps` / `_cap` | 1,000,000 / 10 / 50,000,000 | [0, 10,000,000] / [0, 100] / [0, 250,000,000] | lamports / bps of B̂_ref / lamports | G→P | PROPOSED (C0-1) |
| `util_kink_bps` / `util_cap_bps` | 8,000 / 9,000 | kink ≤ cap ≤ 9,500 | bps | M (own version) / G | SPEC |
| `wallet_cap_bps` | 2,500 | [500, 10,000] | bps of OI cap | G | ADOPTED Q10 |
| `curve_oi_gamma_bps` | 5,000 | [1,000, 9,000] | bps of rt | G | PROPOSED |
| `fee_size_cap_x100` | 300 | **[100, 500]** (no "off") | 1/100 × φ | G | PROPOSED (ECON N2) |
| `fee_oi_cap_x100` | 750 | **[300, 1,000]** (no "off") | 1/100 × φ | G | PROPOSED (ECON N2) |
| `min_notional` | 100,000,000 | [10,000,000, 10⁹] | lamports | G | ADOPTED Q11 |
| `market_max_notional` | 25,000,000,000 (beta) | **[10⁹, 10¹⁴]** | lamports | G | ADOPTED Q21 (finite maximum, R3A-11 a) |
| `market_creation_fee` | 5,000,000 | [0, 100,000,000] | lamports | G | ADOPTED Q24 |
| `s3_wait_s` / `grad_grace_s` | 600 / 600 | [600, 86,400] | s | G→P | SPEC |
| `S3_OBS_GAP_S` | 60 | – | s | C | PROPOSED (C0-16) |
| `max_migration_pause_s` | 3,600 | [600, 86,400] | s | G→P | ADOPTED Q5 |
| `boost_dust_lamports` / `boost_window_s` | 50,000,000 / 900 | [0, 10⁹] / [0, 7,200] | lamports / s | G | ADOPTED Q15 (C1-2) |
| `open_ema_dev_bps` / `market_warmup_s` | 1,000 / 50 | [100, 5,000] / [10·`EMA_TAU_S`, 3,600] | bps / s | G | PROPOSED |
| **`EMA_TAU_S`** | **5** | – | s | **C** | PROPOSED (R3B-6; v0.4.1: τ = 6 loses 83 of the 1,655 battery runs, ECON §5.7) |
| `ema_dtmax_s` / `ema_clamp_bps` | 2 / 10,000 | **[1, 2] / [5,000, 10,000]** | s / bps | M | PROPOSED |
| `ema_force_delay_s` | 604,800 | [86,400, 2,592,000] | s | G | PROPOSED (V2-13) |
| `RING_LEN` (ECON `ring_len`) | 8 | – | entries | C | PROPOSED |
| `ref_max_age_s` | 2 | **[1, 3]** | s | G→P | PROPOSED |
| `liq_persist_n` | 3 | **[2, 3]** | ring entries | G→P | PROPOSED (`liq_persist_s` removed; v0.4.1: 4 loses 58 battery runs) |
| `settle_persist_n` / `settle_arm_gap_s` | 40 / 10 | **[40, 600]** / [2, 60] | EMA updates / s | G→P | PROPOSED (`settle_persist_s` removed) |
| `kappa_liq_bps` / `kappa_base_bps` / `kappa_growth_n` | 2,500 / 0 / 3 | **[2,500, 3,000]** / [0, 1,000] / **[3, 4]** | bps / bps / updates | G→P | PROPOSED (R3B-6; v0.4.1: κ = 2,250 loses 38 battery runs) |
| **`delta_liq_bps`** | **200** | **[200, 500]** | bps over `B̂_ref` | G→P | PROPOSED (decision 1; replaces `delta_liq_min/slope/max_bps`) |
| `delta_exp_bps` | 100 | **[50, 300]** | bps | G→P | PROPOSED |
| `exec_tol_bps` (ε) | 10 | [0, 100] | bps | G→P | PROPOSED; **enters only the expiry bound** since v0.4.1 |
| `exec_rent_tol` (ert) | 6,500,000 | **[6,000,000, 10,000,000]** | lamports | G→P | PROPOSED; must cover the 5,811,520 worst case; the position stores the effective value (0 on Raydium) |
| `fv_dev_bps` | 100 | **[50, 200]** | bps | G→P | PROPOSED: the settlement-box tolerance for S2/S3 amounts (A65) |
| `liq_slow_tau_s` / `_dtmax_s` | 3,600 / 60 | [600, 86,400] / [1, 600] | s | M | PROPOSED |
| **`lump_vest_s`** | **21,600** | **[3,600, 604,800]** | s | G | PROPOSED (R3B-2) |
| **`lock_grace_s`** | **86,400** | **[3,600, 604,800]** | s | G | PROPOSED (R3B-1); added after `max_release`, which contains each position's S3 tail (v0.4.1, R4A-8, R4B-8) |
| **`LOCK_BUCKET_S` / `LOCK_BUCKETS`** | **86,400 / 128** | – | s / buckets | C | PROPOSED (v0.4.1 lock expiry calendar, §2.3.6); compile-time assert `LOCK_BUCKETS·LOCK_BUCKET_S ≥ max_duration_s max + 3·86,400 + lock_grace_s max + 2·86,400` |
| `virtual_shares` / `virtual_assets` | 1,000 / 1 | – | – | C | PROPOSED |
| `R_MAX` / `REBASE` | 10⁶ / 1,000 | – | shares per raw token / factor | C | PROPOSED (C0-6) |
| **`P_ACC_MAX` / `P_OPEN_MAX`** | **10²⁹ / 10²⁷** | – | WAD | C | PROPOSED (ECON §2.2) |
| `theta_A` / `theta_B` | $500 k / $100 k (stored as USD·10⁶) | [$50 k, $50 M] / [$10 k, θ_A) | USD of pool liquidity | G | SPEC; oracle ADOPTED Q8 |
| `AGE_C_S` / `AGE_A_S` | 86,400 / 604,800 | – | s | C | SPEC §4 |
| `oracle_max_age_s` / `oracle_stale_s` | 120 / 3,600 | [60, 600] / [600, 86,400] | s | G | ADOPTED Q8 (C1-12) |
| `oracle_max_conf_bps` / `oracle_max_step_bps` | 200 / 2,000 | [10, 1,000] / [500, 5,000] | bps | G | PROPOSED (A10) |
| `venue_fee_cap_bps` | 150 | [25, 300] | bps, one-way total venue fee | G | PROPOSED (A43; compared with the fee in ppm as `cap·100`) |
| `venue_fee_hard_cap_bps` | 500 | [300, 2,000] | bps | G→P | PROPOSED (A43). The position's value caps every risk quote and FV; the **global** value drives the market-level predicates `s3_no_route` and `pool_fail` (R3A-4 c) |
| `pool_min_real_sol` | 25,000,000,000 | [10⁹, 10¹³] | lamports | G | PROPOSED (A42) |
| `pool_lock_min_bps` (λ_min) | 5,000 | [2,000, 10,000] | bps of LP permanently locked | G | PROPOSED (A42) |
| `pool_switch_margin_bps` / `pool_list_window_s` / `pool_list_margin_bps` | 2,500 / 3,600 / 500 | [500, 10,000] / [0, 86,400] / [0, 2,500] | bps / s / bps | G | PROPOSED (A42); `pool_switch_delay_s` removed |
| `pool_fast_route_s` / `pool_fast_min_bps` / `pool_fast_window_s` | 120 / 2,500 / 60 | **[30, 420]** / [1,000, 10,000] / **[10, 120]** | s / bps of incumbent D / s | G | PROPOSED (A42, A71; R3A-11 a, b) |
| `T_CLAMP` | 1,717,200,000 (2024-06-01T00:00Z) | – | unix s | C | PROPOSED (CPMM `open_time` clamp cut-off) [R-RAY §7] |
| `COVERAGE_GAP_S` | 30 | – | s | C | ARCH-only (event threshold of `CoverageGap`, no economic effect; ECON §5.8) |

**Removed in v0.4** (ECON §15): `ema_tau_s` as a parameter, `delta_liq_min_bps`, `delta_liq_slope_bps`, `delta_liq_max_bps`, `liq_persist_s`, `settle_persist_s`, `fv_liq`, `settle_s2_first`, `margin_ema_own_sale`, `pool_switch_delay_s`, and the ARCH-only `venue_sync_max_age_slots`.

**Relational checks in `validate_params()`** (= ECON §15; v0.4 additions marked ★, v0.4.1 ★★; `market_warmup_s ≥ 10·EMA_TAU_S` and `theta_B < theta_A` involve parameters the sim does not model and are `params::validate_econ`'s, every other one is `params::validate`'s):

- `cr_open_bps ≥ cr_liq_bps + 2,000`; `cr_liq_bps ≥ 10,500`; ★★ per tier, `cr_open_bps` and `cr_liq_bps` ≥ their SPEC §4 values; `util_kink_bps ≤ util_cap_bps ≤ 9,500`; penalty split sums to 10,000;
- `market_warmup_s ≥ 10·EMA_TAU_S`; `venue_fee_cap_bps < venue_fee_hard_cap_bps`; `settle_keeper_fixed ≤ settle_keeper_cap`; `expiry_keeper_fixed ≤ expiry_keeper_cap`; `theta_B < theta_A`;
- ★ `kappa_base_bps ≤ kappa_liq_bps`; ★ `delta_liq_bps ≥ delta_exp_bps`; ★ `fee_size_cap_x100 ≤ fee_oi_cap_x100`; v0.4's `liq_persist_n + kappa_growth_n ≤ 7` is implied by the v0.4.1 bounds and dropped;
- ★ `pool_fast_route_s + pool_fast_window_s ≤ 540`, checked against the **compiled minimum** of `s3_wait_s` (600) minus `S3_OBS_GAP_S`, so no position — whatever `s3_wait_s` it snapshot — reaches S3 before the fast path can complete (R3A-11 b);
- `RING_LEN ≥ max(liq_persist_n, kappa_growth_n + 1)` (holds for every compiled value; asserted at compile time).

**What "simulated-safe" covers (v0.4.1, R4A-4).** The ECON §5.7 battery has 1,655 runs: the 65 core rows, the same rows with ±0.3 % and ±1 % per-second price noise (3 seeds each), the reviewer's 960-cell genuine-pump grid and 240 aged-position runs. Every corner of the core box (κ, persistence, growth window, `dt_max`, clamp, bucket age, δ: 128 configurations that pass the relational checks) passes all of it; every other parameter with a liveness effect passes at each of its compiled extremes alone (the scan; sizing parameters compared at equal position size), except those whose only effect is lower lender income (penalty, expiry fee, rates), which the gate reports as warnings for the owner; and a stress corner with every scanned parameter at its adverse extreme together passes. Combinations beyond the core box are not enumerated. Governance can move a value only inside the bounds; the fuzz harness proposes adversarial in-bound configs (§8.4).

`TierParams` (A / B / C / D). All SPEC or derived from SPEC per ECON §15; the `→P` rows are snapshot into each position. `cr_open_bps` is used only at open, so it is not snapshot.

| Param | A | B | C | D | Hard bounds | Scope |
|---|---|---|---|---|---|---|
| `m_bps` | 10,000 | 15,000 | 20,000 | 30,000 | informational | T |
| `cr_open_bps` | 20,000 | 25,000 | 30,000 | 40,000 | **≥ the tier's SPEC value** (v0.4.1), ≥ cr_liq + 2,000, ≤ 100,000 | T |
| `cr_liq_bps` | 12,500 | 13,500 | 15,000 | 17,500 | **[the tier's SPEC value, cr_open − 2,000]** (v0.4.1) | T→P |
| `penalty_bps` | 500 | 700 | 800 | 1,000 | [0, 2,000] | T→P |
| `max_duration_s` | 2,592,000 | 1,209,600 | 604,800 | 259,200 | [3,600, 7,776,000] | T→P (as `deadline`) |
| `impact_pos_bps` / `impact_mkt_bps` | 500 / 2,000 | 500 / 2,000 | 500 / 2,000 | 300 / 1,000 | [0, 1,000] / [0, 5,000] | T |
| `max_liq_slip_bps` (σ_max) | 2,000 | 2,000 | 2,000 | 3,000 | [500, 5,000] | T→P |
| `rate_base_bps` | 1,000 | 2,500 | 5,000 | 10,000 | ≤ 50,000 | T→P |
| `rate_slope1_bps` (per 100 % U) | 5,000 | 9,375 | 18,750 | 37,500 | ≤ 1,000,000 | T→P |
| `rate_slope2_bps` (per 100 % U) | 100,000 | 150,000 | 300,000 | 550,000 | ≤ 1,000,000 | T→P |
| `min_rent_s` | 3,600 | 3,600 | 3,600 | 14,400 | [0, 86,400] | T→P (as `r_min`) |

#### 2.3.4 `Market` (zero-copy, 3,104 bytes)

Exact layout, computed field by field (every offset a multiple of its alignment, all padding explicit, identical on sbpfv3, x86_64 and wasm32; T-LAYOUT asserts each offset). Offsets include the 8-byte discriminator.

| Group | Offset | Bytes | Fields (in order) | Notes |
|---|---|---|---|---|
| header | 8 | 24 | `version` u16; `flags` u16; `bump`, `pool_vault_bump`, `market_vault_bump`, `trader_bump`, `trader_wsol_bump`, `venue`, `tier`, `decimals`, `ring_head`, `ring_count` u8; `_pad0` [u8;2]; `created_at` i64 | `flags`: bit0 `paused_opens`, bit1 `mint_unsafe`, bit2 `route_broken` (Emergency, A40: opens and S3 evidence only), bit3 `sol_is_token0_or_coin` (Raydium side), bit4 `graduated_elsewhere`, bit5 `wipe_live` (V2-12), ★bit6 `paused_coverage` (degraded mode, A69), ★★bit7 `wipe_trader_swept`, ★★bit8 `wipe_rewards_claimed` (cleared at a full wipe, set by `sweep_trader` / `claim_venue_rewards` while `S = 0`; `deposit` into a live wipe record requires both, the second only on pump venues that can execute; R4A-7) |
| identity | 32 | 96 | `mint`, `token_program`, `creator` Pubkey | Every instruction asserts `mint.key`, `mint.owner` and `token_program` against these [R-MINT §6] |
| own accounts | 128 | 160 | `pool_vault`, `market_vault`, `trader`, `trader_base_ata`, `trader_wsol` | checked by key equality |
| pump detection keys | 288 | 64 | `bonding_curve`, `canonical_pool` | **Always derived and stored at creation**, for every market (C0-15) |
| curve venue cache | 352 | 256 | `curve_base_ata`, `curve_quote_ata`, `sharing_config`, `pump_uva`, `pump_uva_quote_ata`, `cached_creator`, `cached_creator_vault`, `cached_creator_vault_ata` | `creator_vault` re-derived only if `curve.creator ≠ cached_creator` [R-PUMP §8] |
| pool venue cache | 608 | 320 | `[Pubkey;10]`, union by `venue`: PumpSwap `pool_base_vault`, `pool_quote_vault`, `pool_v2_marker`, `boost_vault_ata`, `pamm_uva`, `pamm_uva_quote_ata`, `cached_coin_creator`, `cached_coin_creator_vault_auth`, `cached_coin_creator_vault_ata`; Raydium `ray_pool`, `ray_vault_sol`, `ray_vault_tok`, `ray_amm_config`, `ray_observation`, `ray_lp_mint`, `ray_tok_program` | |
| pool selection | 928 | 248 | `pool_seen_at` i64; `pool_d` PodU128 (incumbent locked depth, refreshed by `crank_price` and `propose_pool` while the incumbent is executable); fast-path window: `fp_keys` [Pubkey;6], `fp_d` PodU128 (best candidate seen), `fp_t0` i64, `fp_venue` u8, `_pad1` [u8;7] | The v0.3 non-fast pending switch is gone (decision 7) |
| venue timing | 1,176 | 24 | `migration_seen_at`, `graduated_at`, `tier_computed_at` i64 | 0 = unset. `graduated_at` persists across pool switches |
| route chains | 1,200 | 32 | `no_route_since`, `last_no_route_obs` (S3 chain, `s3_no_route`); ★`pool_fail_since`, `last_pool_fail_obs` (fast-path chain, `pool_fail`) i64 | ECON §10.3; R3A-4, R3B-8 |
| lender pool (market part) | 1,232 | 192 | `tokens_free` (T), `borrowed_q` (ΣB_Q) u64; `borrowed_q_by_tier` [u64;4]; `total_shares` (S) PodU128; `acc` PodI128; `carry` PodI128; `acc_r` PodU128; `carry_r` PodU128; `dust` PodI128; `receivable` u64; ★★`max_release` i64 (v0.4 `max_deadline`); ★`vest_bal` u64, `vest_last` i64, `vest_end` i64; `share_epoch` u32; `_pad2` [u8;4] | Mirrors `sim/pool_model.py` v0.4.1; the lock state, its calendar and the pot live on `MarketVault` (§2.3.6); `unassigned_sol` removed. `max_release = max over positions opened of (deadline + max_migration_pause_s + grad_grace_s + s3_wait_s)`, snapshot values (R4A-8) |
| rent (§4.5) | 1,424 | 104 | `rent_basis` [PodU128;3] (J0, J1, J2); `rent_aggr` [PodU128;3] (A0, A1, A2 = Σ Q·coef_j·(10⁴ − p_pos)); `last_accrual_ts` i64 | A36; the protocol share is folded into `A_j` (R3A-5) |
| observation (ECON §5) | 1,528 | 1,272 | bucket `b_sec` i64, `b_lo`, `b_hi`, `b_llo`, `b_lhi` PodU128; last completed bucket `lb_sec` i64, `lb_lo`, `lb_hi` PodU128; `ema_price`, `ema_liq`, `liq_slow` PodU128; `ema_ts`, `slow_ts` i64; `ema_count` u64; `ring` [RingEntry;8] | `RingEntry` = `ts` i64, `p`, `l`, `x`, `y`, ★★`lo`, `hi`, `llo`, `lhi` PodU128 = **136 B**: `(x, y) = reprice(l, p)` cached at integration; `(lo, hi, llo, lhi)` = the integrated bucket's price and liquidity extremes, which replace v0.4's clamped `obs`, `lobs` (the settlement box's medians, R4B-2) |
| market-level params | 2,800 | 32 | `ema_params_version`, ★`kink_version`, `ema_clamp_bps`, `util_kink_bps`, `liq_slow_tau_s`, `liq_slow_dtmax_s` u32; `ema_dtmax_s` u16; `_pad3` [u8;6] | τ is `EMA_TAU_S` (not stored) |
| OI | 2,832 | 16 | `open_positions` u32; `_pad4` [u8;4]; `total_notional_open` u64 | |
| misc | 2,848 | 256 | `_reserved` [u8;256] | |

Rent-exempt minimum 16,418,560 lamports (v0.4: 2,848 B, 15,118,080 lamports). The offsets are generated by `docs/design/arch/layout.py` (which fails on any implicit padding) and must equal the T-LAYOUT assertions.

#### 2.3.5 `ShortPosition` (Borsh `Account<T>`, 384 bytes)

| Field | Type | Bytes | Notes |
|---|---|---|---|
| discriminator | — | 8 | |
| `borrower`, `market` | Pubkey | 64 | |
| `nonce` | u64 | 8 | Chosen by the client; `init` fails if reused |
| `bump`, `status`, `tier`, `venue_at_open` | u8 ×4 | 4 | `status`: 0 = Open (1 = ExitInProgress reserved for v1.1) |
| `q` | u64 | 8 | Tokens owed; constant (no partial close in MVP, SPEC §8) |
| `s`, `m`, `v` | u64 | 24 | Sale proceeds, margin, tracked vault lamports (`v` changes only through `add_margin`) |
| `n_open`, `fee_open` | u64 | 16 | ECON §3.3, §8.7 |
| `entry_price` | u128 | 16 | `S·WAD/Q` |
| `j_open` | `[u128;3]` | 48 | Rent basis indices at open (§4.5) |
| `r_min` | u64 | 8 | Minimum rent, fixed at open (ECON §2.1) |
| `t_open`, `deadline` | i64 | 16 | `deadline = t_open + max_duration_s[tier]` |
| **locked tier terms** | `cr_liq_bps` u32, `penalty_bps` u16, `max_liq_slip_bps` u16, `expiry_fee_bps` u16, `rate_base_bps` u32, `rate_slope1_bps` u32, `rate_slope2_bps` u32 | 22 | ADOPTED Q9 |
| **locked split, rent share and keeper terms** | `penalty_split` `[u16;3]`; ★`rent_protocol_bps` u16; `settle_keeper_fixed` u64, `settle_keeper_bps` u16, `settle_keeper_cap` u64; `expiry_keeper_fixed` u64, `expiry_keeper_bps` u16, `expiry_keeper_cap` u64 | 44 | C0-8; R3A-5 |
| **locked exit-bound and persistence terms** | ★`delta_liq_bps`, `delta_exp_bps`, `kappa_liq_bps`, `kappa_base_bps` u16; `kappa_growth_n`, `liq_persist_n` u8; `settle_persist_n`, `settle_arm_gap_s` u16 | 14 | C0-8; v0.4 removals: `delta_liq_min/slope/max_bps`, `liq_persist_s`, `settle_persist_s` |
| **locked execution and settlement terms** | `exec_tol_bps` u16 (the expiry bound's ε, v0.4.1); `ert` u64 (effective: 0 on Raydium); `ref_max_age_s` u8; `fv_dev_bps` u16; `venue_fee_hard_cap_bps` u16; `s3_wait_s`, `grad_grace_s`, `max_migration_pause_s` u32 | 27 | V1-5; v0.4 removals: `fv_liq`, `settle_s2_first`; the three timers also feed `Market.max_release` at open (v0.4.1) |
| **settlement arming state** | `s_armed` u8 (0/1); `s_count` u64 (`ema_count` at arm); `s_since`, `s_last` i64 | 25 | One clock for S1 and S2; §3.8 |
| `_reserved` | `[u8;32]` | 32 | Room for the O1/O2 terms if the owner ever signs them off (they are not built, R3A-8 c) |

Rent-exempt minimum 2,600,960 lamports. **A26: what "locked at open" covers, and what governance can still affect (V1-5; amended R3A-4, R3A-5).** Everything in the four **locked** groups plus `deadline` and `r_min` never changes for the life of the position; the rent rate **and the protocol's share of it** are locked exactly through the basis indices and `A_j` (§4.5). What a timelocked governance change can still reach while a position is open, always inside compiled bounds:

- **market-level observation parameters** (`ema_dtmax_s`, `ema_clamp_bps`, `liq_slow_*`): adopted when the market has no open position, or after `ema_force_delay_s` (≥ 1 day, default 7 days); their compiled ranges keep a forced adoption inside the simulated-safe region (ECON §2.4). `util_kink_bps` only when the market has no open position;
- **the global `venue_fee_hard_cap_bps`** as it enters the market-level predicates `s3_no_route` (the S3 chain and `sol_settle`'s "no route now") and `pool_fail` (the fast path): a change inside [300, 2,000] bps can move S3 and fast-path timing for open positions; it never changes their quotes, bounds or FV, which use the locked value (R3A-4 c);
- **Raydium pool-selection parameters** (`pool_*`): they decide which eligible pool an open position exits on after an incumbent failure (§2.6);
- **Emergency flags**: the route-broken flag and the venue-extras sets, which never change a quote or an exit bound (A40, A41).

Liquidation thresholds, penalties, splits, the protocol's rent share, keeper rewards, exit bounds, persistence and arming rules, fee and tolerance ceilings, fair-value rules and S3 timing terms are all per-position (C0-8).

#### 2.3.6 `LenderPosition`, `ShareEpoch`, `MarketVault`, `WalletExposure`, `OracleCache` (Borsh)

- **`LenderPosition` (241 B)**: `owner`, `market` Pubkey; `shares` u128; `snap` i128 (acc snapshot); `pending` i64 (entitlement not yet paid; can go negative after a shortfall debit, ECON §9.4); `snap_r` u128 (acc_r snapshot); `real` u64 (cumulative realised SOL allocated to this lender, including pot draws) and `claimed` u64 (cumulative SOL paid to it) (V2-2); `epoch` u32; `lock` u128, `lock_pi` u128, `lock_epoch` u32, `lock_term` u128 (ECON §9.3); ★`lock_until` i64 (R3B-1); `bump` u8; `_reserved [u8;32]`.
- **`ShareEpoch` (127 B)** (A39): `market` Pubkey; `epoch` u32; `acc_end` i128; `acc_r_end` u128; `s_old` u128 (shares at the roll) and `carry` i128, `carry_r` u128 (top-up remainders); `num` u8 (0 or 1); `den_log1000` u8 (den = 1,000^k); `bump`. Immutable once written, except the full-wipe case of §3.3 (V2-12).
- **`MarketVault` (zero-copy, 2,200 B; v0.4 was 77 B Borsh)** (A9, C0-17; amended A70, A75): `lender_sol` u64 (ECON `sol_vault`: realised lender SOL, including the pot and the unreleased vest); ★`unmatched` u64 (the pot, R3A-2); `protocol_fees` u64; the lock state `pi` PodU128 (Π, WAD), `lock_norm` PodU128 (Σ of the live calendar buckets), `b_min` u64, `pi_epoch` u32, `bump` u8, `_pad0` [u8;3]; ★★the **lock expiry calendar** `cal_day` i64 (the oldest day not yet dropped) and `lock_cal [PodU128; 128]` (bucket `d mod 128` holds the lock terms whose `lock_until` ends day `d`); `_reserved [u8;64]`. Rent-exempt minimum 11,826,240 lamports. Its lamports are the SOL float. **Calendar (A75; R4A-2, R4B-3):** every instruction that reads or writes the lock state first runs `cal_advance(now)`: for each day `d` from `cal_day` to `⌊now/86,400⌋` (at most 128 iterations; a longer gap clears every bucket), `lock_norm −= lock_cal[d mod 128]`, `lock_cal[d mod 128] = 0`; then `cal_day = max(cal_day, ⌊now/86,400⌋ + 1)` (never backwards, should the clock go back). `lock_until` is always a day end, so a bucket leaves `lock_norm` at exactly the second its locks expire for their holders — no lender has to be touched. A Π-epoch roll clears every bucket; a rebase scales every bucket by `num/den`. ECON §9.1 lists the lock state with the market's logical state; ARCH stores it here because every instruction that changes it (withdrawals, exits, settlements, lender touches) already write-locks `MarketVault`, which lets `claim_sol`, `cover_deficit` and `sync_lender` run pool_model's full `touch` without write-locking `Market` (C0-17). v0.4.1 makes the account zero-copy because the calendar (2 KB) would cost a Borsh round trip on every claim. **The isolation is from `Market` only** (V1-6): exits, opens and lender instructions all write-lock `MarketVault`, so lender claims and liquidations still share its 24 M CU per-block budget; the mitigation is the keeper's CU and priority policy and T-CASCADE (§6.6).
- **`WalletExposure` (81 B)**: `market`, `wallet` Pubkey; `q_open` u64; `bump`.
- **`OracleCache` (45 B)**: `price` i64, `conf` u64, `expo` i32, `publish_time` i64, `updated_slot` u64, `bump` [R-VEN §5].

### 2.4 Funds, vaults and signer-authority isolation

```
                      (signs transfer_checked / approve_checked(Q) only)          (never sign; program-owned lamports)
 lender ATA ──transfer_checked──► pool_vault [authority = market PDA]            ShortPosition.lamports = rent + V
                                    │  ▲   ▲                                      MarketVault.lamports   = rent + lender_sol + protocol_fees
     pump venues: open Q out ───────┘  │   │── Raydium buy: venue delivers Q straight into pool_vault
     pump venues: exit Q in ◄──────────┘   │── Raydium sell: trader spends Q as delegate of pool_vault (approve Q, auto-cleared at 0)
                                    │
                         trader_base_ATA (pump venues only) ◄── owner = trader PDA (system-owned, 0 data, 0 lamports after every ix)
                                                                   │  ▲
         pump sell_v2 credits lamports / pump buy_v2 debits them ──┘  │── direct lamport credit from ShortPosition (budget), System sweep back
                                    trader_wsol (ephemeral, PDA address, owner = trader): PumpSwap and Raydium SOL leg,
                                    created with allocate/assign + InitializeAccount3 and closed in the same instruction
                                         VENUE PROGRAM (compile-time constant): the ONLY program that receives the trader signature
```

**Isolation rules** (A4, A33; implement [R-JUP §4.3], [R-MINT §8.5], [R-RAY §9.3]).

1. **Signers are fixed per program.**
   - The `market` PDA signs only Token `transfer_checked` out of `pool_vault` (withdraw; open leg on pump venues) and `approve_checked(pool_vault → trader, Q)` (Raydium sell leg).
   - The `trader` PDA signs only: the CPI into the market's venue program (a compile-time constant for the venue kind); System transfers of its own lamports; Token `transfer_checked` from its base ATA to `pool_vault`; `CloseAccount` of `trader_wsol` or of a stray `ATA(trader, WSOL)`; pump/PumpSwap `claim_cashback` (§3.9).
   - The `trader_wsol` PDA signs only System `transfer` (sweeping pre-funded lamports), `allocate` and `assign` of itself.
   - `ShortPosition`, `MarketVault`, `Market` and `ShareEpoch` never sign and are never passed to a venue program.
2. **Normalise before measuring** (A33; C0-3, C1-3). Anyone can credit the system-owned trader (a System transfer checks only `from`), create token accounts for it (the ATA program checks only the address), or pre-fund the `trader_wsol` address [R-PUMP §9.2; R-RAY §4.2]. At the start of every venue leg, before any funding:
   - require `trader.owner == System ∧ trader.data_len == 0` (only short.win can sign `allocate`/`assign` for it, so this cannot be griefed);
   - if `trader.lamports > 0`: System-transfer all of it to `market_vault` (trader signs) and credit it to lenders as a lump (`lender_sol += x`, `pool::sweep(x)`: it vests over `lump_vest_s`; while `S = 0` it tops up the live wipe record or goes to `protocol_fees`, §3.3); emit `TraderSwept`;
   - pump venues: if `trader_base_ata.amount > 0`: `transfer_checked` all of it to `pool_vault` (a donation; `T` is unchanged); require `owner == trader`, `delegate == None`, `close_authority == None` (only the trader could set them);
   - WSOL venues: if the `trader_wsol` address holds lamports, System-transfer them to `market_vault` (signed with the `trader_wsol` seeds) and credit lenders the same way; require `owner == System ∧ data_len == 0`;
   - `ATA(trader, WSOL)` is never used by a trading leg; if a third party creates it, `sweep_trader` (§3.9) closes it later. Pump ignores it for SOL coins [R-PUMP §6.3].

   `S` and `B` are measured against this post-normalisation baseline. Post-conditions assert **deltas and authorities**, never that an account a third party can create does not exist.
3. **Exact funding and balance-delta accounting** (A19, A48; C0-14, C1-4, C1-9). `ert` = `exec_rent_tol` on pump venues, 0 on Raydium.
   - **Buy legs**: fund the leg with `max_in + ert` (pump curve: native lamports on the trader; PumpSwap: `max_in` as WSOL in `trader_wsol` plus `ert` native on the trader; Raydium: `max_in` as WSOL in `trader_wsol`). The source is the `ShortPosition` itself (direct lamport debit, no CPI), capped at its total lamports. Rent state is checked once per transaction [V4], so briefly using the position's rent reserve is safe. After the CPI, everything left returns to the position in the same instruction.
     - `B = position.lamports_before − position.lamports_after`. Require `B ≤ max_in + ert` and `tokens_received ≥ Q` (measured on `trader_base_ata` for pump venues, on `pool_vault` for Raydium). Under-delivery reverts; **over-delivery** (a venue rounding or semantics change) is accepted: the surplus goes to `pool_vault` as a donation (`T` unchanged, I1 already tolerates it) and is emitted, so a harmless venue change cannot freeze exits (A62; V2-10).
   - **Sell legs (open)** — one baseline for both pump venues (V1-13). Pump venues fund the trader with `ert` from the borrower **before** the CPI, because `sell_v2` may top up the creator vault, the curve account and the UVA from `user`, and PumpSwap `sell` may create missing fee accounts (C1-4; **UNVERIFIED** which, T-RENT). The baseline is the post-normalisation, post-funding state: trader lamports = `ert`, no `trader_wsol` balance.
     - `S = trader.lamports_after + trader_wsol.amount_after − ert` on both pump venues (curve: lamports only; PumpSwap: WSOL plus lamports). Any venue-side rent `r` the venue took from the trader is therefore netted inside `S`, and the borrower is refunded exactly `ert`. The borrower bears `r` through a smaller `S`, and hence a larger margin. On PumpSwap `r = ert − trader.lamports_after` is reported in the `Open` event; on the curve `r` cannot be separated from the proceeds.
     - The venue's own slippage argument (`min_sol_output` / `min_quote_out`) is `min_out` and applies to the venue's gross credit. short.win then requires the **net** `S ≥ min_out`, with `min_out = max(min_proceeds, ⌊S_exp·(10⁴ − ε)/10⁴⌋)`, so the borrower's `min_proceeds` bounds what is actually credited to the position.
     - Raydium: `ert = 0` and `S = trader_wsol.amount`; the borrower funds and gets back only the `trader_wsol` rent.
     - Require the pool-vault base delta to be exactly `−Q` (short.win's own transfer or approval).
4. **Account denylist.** A remaining account must never equal `config`, `venue_guard`, `market`, `market_vault`, `pool_vault` (except where the venue list names it: Raydium source/destination), any `ShortPosition`, `LenderPosition`, `ShareEpoch` or `OracleCache` (O(n) key scan). CPI metas set `is_signer = true` only for `trader` (or `swap_auth` in v1.1). Top-level signer flags are never forwarded [R-JUP T8]. Extra trailing accounts (A41) must also not be owned by short.win.
5. **Post-conditions at the end of every instruction that ran a leg** (invariant I7): `trader.lamports == 0`; `trader_base_ata.amount == 0` with owner/delegate/close-authority unchanged; `trader_wsol` closed (`owner == System ∧ data_len == 0 ∧ lamports == 0`); `pool_vault.delegate == None` [V12]; `pool_vault.amount ≥ T`; the vault lamport invariants of §2.3.1.
6. **Token movements use `transfer_checked` / `approve_checked`** through `anchor_spl::token_interface`, with `market.token_program` [R-MINT §8.3].

**Why a per-market trader** (A4): pump and PumpSwap each need a pre-created UVA per `user` (1,346,200 lamports) [R-PUMP §2.4]; the trader holds nothing at rest, so there is no cross-position exposure. The v1.1 Jupiter adapter uses a per-position `swap_auth`, because a Jupiter route passes the signature on to arbitrary AMMs.

#### 2.4.1 Leg flows per venue

Trace entries are counted per leg (C1-1): the short.win instruction itself is 1; every CPI and every CPI it makes is 1 more. Normalisation adds 0–3 entries (only when something was donated). Venue counts are measured on mainnet [R-PUMP §6.9; R-RAY §4.3].

| Venue | Sell leg (open, exact in) | Buy leg (close / liquidate / expire, exact out) |
|---|---|---|
| **PumpCurve** | 1. `transfer_checked(pool_vault → trader_base_ata, Q)` (market signs). 2. System `borrower → trader`, `ert`. 3. CPI `sell_v2(Q, min_out)` (4 entries). 4. `S = trader.lamports − ert`; System `trader → position` `S`; System `trader → borrower` `ert`. **Trace ≈ 1 + 1 + 1 + 4 + 2 = 9** | 1. Direct debit `position → trader`, `max_in + ert`. 2. CPI `buy_v2(Q, max_sol_cost = max_in)` (8). 3. System `trader → position` (all). 4. Require `trader_base_ata.amount ≥ Q`; `transfer_checked(trader_base_ata → pool_vault, all)` (trader signs). **Trace ≈ 1 + 8 + 2 = 11** |
| **PumpSwap** | 1. `transfer_checked(pool_vault → trader_base_ata, Q)`. 2. System `borrower → trader` `ert`; System `borrower → trader_wsol address` `rent(165)`. 3. Create `trader_wsol` (allocate, assign, `InitializeAccount3(WSOL, owner = trader)`). 4. CPI `sell(Q, min_quote_out)` (8). 5. `S = trader_wsol.amount + trader.lamports − ert`; `CloseAccount(trader_wsol → position)`; System `trader → borrower` (all, `= ert − r`); direct debit `position → borrower` `rent(165) + r`. **Trace ≈ 1 + 1 + 2 + 3 + 8 + 2 = 17** | 1. Direct debit `position → trader` `ert` and `position → trader_wsol address` `rent(165) + max_in`. 2. Create `trader_wsol` (3). 3. CPI `buy(Q, max_quote_in = max_in, track_volume = [false])` (8). 4. `CloseAccount(trader_wsol → position)`; System `trader → position` (all). 5. Require `trader_base_ata.amount ≥ Q`; `transfer_checked(trader_base_ata → pool_vault, all)`. **Trace ≈ 1 + 3 + 8 + 3 = 15** |
| **RaydiumV4 / RaydiumCpmm** | 1. `approve_checked(pool_vault → trader, Q)` (market signs). 2. System `borrower → trader_wsol address` `rent(165)`. 3. Create `trader_wsol` (3). 4. CPI exact-in swap, `source = pool_vault` (trader as delegate), `destination = trader_wsol` (3). 5. `S = trader_wsol.amount`; require `pool_vault` delta `−Q` and `pool_vault.delegate == None` (auto-cleared at 0 [V12; R-RAY §4.2]); `CloseAccount(trader_wsol → position)`; direct debit `position → borrower` `rent(165)`. **Trace ≈ 1 + 1 + 1 + 3 + 3 + 1 = 10** | 1. Direct debit `position → trader_wsol address` `rent(165) + max_in`. 2. Create `trader_wsol` (3). 3. CPI exact-out swap, `source = trader_wsol`, `destination = pool_vault` (the venues do not check the destination owner [R-RAY §3.1–3.2, SIM]) (3). 4. Require `pool_vault` delta `≥ +Q` (surplus: donation, V2-10); `CloseAccount(trader_wsol → position)`. **Trace ≈ 1 + 3 + 3 + 1 = 8** |

Every instruction adds its own `emit_cpi!` event (1 entry) and its margin/fee transfers (open: 2). §6.2 has the per-instruction totals.

**The ephemeral WSOL account (A5).** `trader_wsol` lives at `PDA(short.win, ["trader_wsol", market])`, and its token owner is `trader`.

- **Grief-proof creation** (C0-3): a third party can only pre-fund the address. `System::create_account` would then fail with `AccountAlreadyInUse`, but `allocate` and `assign` only require empty data and the System owner [R-RAY §4.2, agave `system_processor.rs` L75–170]. The helper `create_pda_account(addr, seeds, space, owner)` therefore (1) sweeps any pre-funded lamports to `market_vault`, (2) funds exactly the needed lamports, (3) `allocate(space)` and `assign(owner)` signed with the PDA seeds. `ShareEpoch` records use the same helper.
- `InitializeAccount3` on the native mint sets `amount = lamports − rent`, so no `SyncNative` is needed [R-RAY §4.2].
- A `CloseAccount` on a native account sends all lamports to the destination, which can be the program-owned `ShortPosition` (any writable account can be credited).
- **PumpSwap acceptance is a test gate (T-ATA).** The pAMM IDL puts no ATA seed constraint on `user_quote_token_account` / `user_base_token_account`, and its `claim_cashback` docs say the user's WSOL account "may be any token account of `quote_mint` owned by `user`" [V11]. If the fork test shows PumpSwap nevertheless requires the ATA address, the PumpSwap adapter uses `ATA(trader, WSOL)` instead: `create_idempotent` (payer trader), and if it already existed, close it to the trader first and normalise (so a pre-created account is harmless, C0-3).

**Raydium specifics (A47).** Both Raydium venues need an SPL-Token WSOL account on the SOL side and pay no native SOL [R-RAY §4.2]. No ATA, UVA or fee recipient exists on Raydium, so no venue-side rent and no keeper-chosen accounts (C0-14 does not apply there) [R-RAY §4.1]. The sell source is the lending vault itself, with the trader as delegate for exactly `Q`; the delegate clears automatically when the allowance reaches 0 in both p-token and Token-2022 [R-RAY §4.2; V12]. Whether AMM v4 and CPMM accept a delegate as the transfer authority for the source is verified by the Token program, not by the venue, but it has no mainnet example: **UNVERIFIED**, gate T-RAY-PDA.

### 2.5 Venue state machine

**A1.** This implements D1/D2 and fixes C0-15 (no terminal `None`, no optional detection accounts).

```
enum VenueKind { None = 0, PumpCurve = 1, PumpMigrating = 2, PumpSwap = 3, RaydiumV4 = 4, RaydiumCpmm = 5 }
                 // None = list-only: deposits/withdrawals only, no opens (SPEC §3: CLMM/DLMM-only tokens, no pool, ineligible pool)

classify_pump(bc, cp):                       // bc = PDA(pump, ["bonding-curve", mint]); cp = canonical PumpSwap pool PDA
                                             // BOTH keys are always derived on-chain and required (key-checked) — C0-15
  if bc.owner == pump && bc.disc == BondingCurve:
      require !curve.is_mayhem_mode && curve.quote_mint == Pubkey::default()      // ADOPTED Q6
      require bc.data_len ≥ 115 (else CurveNeedsExtend; client prepends pump extend_account)
      if !curve.complete                        -> PumpCurve
      elif canonical_pool_valid(cp)             -> PumpSwap           (graduated_at = now; A12)
      elif curve.virtual_token_reserves > 0     -> PumpMigrating      (migration_seen_at = now)
      else                                      -> GraduatedElsewhere // complete, reserves 0, no canonical pool (e.g. Fartcoin [V3])
  else                                          -> NotPump

create_market(venue_hint ∈ {Auto, RaydiumV4, RaydiumCpmm}):
  k = classify_pump(bc, cp)
  if k ∈ {PumpCurve, PumpMigrating, PumpSwap}:  require venue_hint == Auto (else PumpVenuePrecedence; D1); venue = k
  elif venue_hint == Auto:                      venue = None     (graduated_elsewhere flag set if k == GraduatedElsewhere)
  else:                                         candidate accounts mandatory; require eligible(candidate) (§2.6);
                                                venue = hint; adopt candidate; graduated_at = age_start(candidate)

sync_venue (prologue of every instruction that carries a ReadSet):
  PumpCurve     : complete ? (cp valid ? PumpSwap : (vT > 0 ? PumpMigrating : None*)) : stay
  PumpMigrating : cp valid ? PumpSwap : (vT == 0 ? None* : stay)
  PumpSwap      : stay
  RaydiumV4/Cpmm: stay (changes only through the pool-selection path, §2.6)
  None          : crank_price re-runs classify_pump (always passed): pump venue found -> that venue (C0-15);
                  a Raydium pool can be adopted through propose_pool (§2.6)
  * A live curve market whose curve completes with zeroed reserves and no canonical pool means pump changed its
    migration target: venue = None, graduated_elsewhere = 1, emit VenueStateChanged. The market has no route, so the
    S3 chain runs (§5.3); a Raydium pool can then be adopted through propose_pool.

canonical_pool_valid(cp) := cp.owner == pAMM && disc == Pool && pool.index == 0
    && pool.creator == PDA(pump, ["pool-authority", mint]) && pool.base_mint == mint
    && pool.quote_mint == WSOL && !pool.is_mayhem_mode                     (the address is derived, so it is unspoofable)

On → PumpSwap:
  cache pool_base_vault/pool_quote_vault (Pool offsets 139/171), pool_v2 marker PDA(pAMM, ["pool-v2", mint]),
  boost_vault_ata = ATA(PDA(pAMM, ["boost_vault", pool]), WSOL), trader_base_ata if missing (created by the keeper
  top-level; see §7.1), graduated_at = now (first observation, conservative; O13), L_ema = L_slow = √(X·Y) of the new pool,
  rewrite every ring entry's l, llo, lhi and cached (x, y) to it (A72; ECON §5.2, §10.1),
  clear the current observation bucket, emit VenueStateChanged.
```

**Precedence (D1).** A pump coin with a live curve or a canonical PumpSwap pool always uses those venues, even if a deeper Raydium pool exists; `create_market` and `propose_pool` reject Raydium candidates for such markets. Whether a deeper Raydium pool should ever win is part of owner item O18 (§10); the build keeps D1 precedence.

**Raydium-graduated pump coins and other Raydium tokens.** These map to `RaydiumV4` / `RaydiumCpmm`, not to `None`. The frontend and the keeper run `eligible()` off-chain (same `shortwin-venue-state` code) and always pass the eligible candidate with the highest `D` as `venue_hint`; `Auto` is for tokens with no eligible pool. A market created as `None` anyway (by anyone) is moved onto its pool by the permissionless `propose_pool`. Example: Fartcoin's curve is complete with zeroed reserves and has no canonical PumpSwap pool [V3], so its market adopts AMM v4 pool `Bzc9NZfM…` (38,100 SOL real, λ 99.78 %) [R-RAY §6.4].

**Is the curve complete and not yet migrated?** Test `complete && virtual_token_reserves > 0`. Just before migration a completed curve still has `vT = 279.9e12`; after migration every reserve is 0 [R-PUMP §2.1, Corrections 1]. Pump coins that graduated to Raydium before PumpSwap show exactly the zeroed state with no canonical pool [V3], and now map to a Raydium venue through the hint or `propose_pool`.

### 2.6 Raydium pool selection (A42 as amended by A60, A71; decision 7)

A mint can have many Raydium SOL pools on both programs; anyone can create one for 0.15 SOL, and neither program derives the pool from the mint [R-RAY §2.1, §2.4, §6.1]. The market must not adopt a thin pool (cheap manipulation), a pool whose liquidity can be pulled, or misread which side is SOL. v0.4 also makes the rule simpler: **a market with open positions changes its pool only when the incumbent fails.**

**Verification of a candidate** (every check on-chain; every account mandatory and key-checked) [R-RAY §6.2]:

- **AMM v4 pool `P`:** `P.owner == 675kPX9…`, `data_len == 752`; `status ∈ {1, 6, 7}`; `nonce == 254` (so the authority is the constant `5Q544f…`); `{coin_vault_mint, pc_vault_mint} == {WSOL, market.mint}` recording which side is SOL (WSOL must be exactly `So111…112`); the passed vaults equal `P.coin_vault` / `P.pc_vault`, are Tokenkeg accounts with the right mints and authority `5Q544f…`; `market.token_program == Tokenkeg` (AMM v4 is SPL-Token only).
- **CPMM pool `P`:** `P.owner == CPMMoo8…`, `data_len == 637`, discriminator `f7ede3f5d7c3de46`; `{token_0_mint, token_1_mint} == {WSOL, market.mint}` recording the SOL side; the WSOL side's token program is Tokenkeg (the Token-2022 native mint is rejected), the token side's program equals the mint owner; the vaults equal `P.token_{0,1}_vault` with the right mints and authority `GpMZb…`; `amm_config == P.amm_config` (owner CPMM, discriminator `daf42168cbcb2b6f`); `observation == P.observation_key`.
- **Both:** the LP mint equals `P.lp_mint` (Tokenkeg; `supply` at offset 36).

**Eligibility** (`eligible(P)`): verification passes; `venue_executable(P)` (§5.3); one-way fee `φ_ppm ≤ venue_fee_cap_bps·100` (§4.2); real SOL reserve `R ≥ pool_min_real_sol` (§4.7); and permanently locked LP share `λ ≥ pool_lock_min_bps`, where

```
λ_bps = ⌊(lp_counter − lp_mint.supply)·10⁴/lp_counter⌋      AMM v4: lp_counter = AmmInfo.lp_amount @720; CPMM: PoolState.lp_supply @333
```

The internal counters change only inside the venue program, and LP burned outside it stays in the counter, so `counter − circulating supply` is LP that can never be redeemed [R-RAY §6.3]. LP held in lockers counts as circulating, which is conservative. The main pools of all seven D2 target coins have λ between 92 % and 99.8 % [R-RAY §6.4].

**Coverage gap (V2-11; owner item O18).** Launchpads that *lock* LP in a locker program instead of burning it produce pools with λ ≈ 0, which are never eligible, so their tokens are list-only despite D2. Live survey on 2026-09-27 [V16]: 1,668 CPMM pools with the creator fee enabled and WSOL as token 0; 311 of them hold ≥ 25 SOL of real reserve, and 67 of those have λ < 50 % — e.g. `AtqUb7Dh…` (2,024.6 SOL, λ 0.0000), `CheJUxVQ…` (608.6 SOL, λ 0.0000), `7H59ffyZ…` (465.5 SOL, λ 0.0000) — while letsbonk-style pools that burn LP pass (`FQd8z3…` 547.2 SOL, λ 0.998; `koZZXrKJ…` λ 1.000). A v1.1 option is to count LP held by the vaults of specific permanent-locker programs as locked, with each program key-checked and its irreversibility proven from source.

**Metric: locked constant-product depth** `D = ⌊λ_bps·isqrt(X·Y)/10⁴⌋` on the pricing reserves (§4.1; ECON §2.1). PROPOSED refinement of [R-RAY §6.3], which used locked SOL `R·λ`:

- A single swap leaves `√(X·Y)` unchanged except for the fee the pool keeps, so an atomic `[buy → propose → sell]` cannot inflate `D` beyond that fee growth. `R·λ` would move with the price.
- **`D` can be pushed only at fee cost** (V2-7). AMM v4 keeps the whole trade fee in the vault until a later `withdrawpnl` [V14; R-RAY §2.3], CPMM keeps the LP part (≈ 84 %) [R-RAY §2.5], so volume grows `√(X·Y)`: a trade of `A` lamports on fee `f` grows it by ≈ `f·A/(2Y)`, and raising `D` by a factor `(1 + x)` through wash trading costs ≈ `2·Y·ln(1 + x)` in fees (+25 %: ≈ 0.45·Y). At least the locked share `λ ≥ λ_min` of those fees stays with burned LP, so an attacker holding the unlocked LP recovers at most `(1 − λ)` of them. Hence the margins below.
- Adding or removing unlocked liquidity changes `√(X·Y)` and `λ` inversely, leaving `D` unchanged. The other way to raise `D` is to burn LP, which permanently donates liquidity to the pool: an honest outcome.
- Two pools of the same pair compare correctly on `√(X·Y)`: it equals the SOL reserve divided by `√P`, so it ranks SOL depth at a common price.

**Rule.**

1. **Listing.** `create_market(venue_hint = RaydiumX)` adopts an eligible candidate; `pool_seen_at = now`. Opens stay disabled until `pool_seen_at + pool_list_window_s` (1 h). During that window `propose_pool` adopts a candidate immediately if `D_cand ≥ (1 + pool_list_margin_bps/10⁴)·D_inc` (5 %: `D` can be pushed only at fee cost, V2-7).
2. **Adoption without a failure — only while `open_positions == 0`** (decision 7). After the window, `propose_pool(candidate)` adopts the candidate immediately if the market has no open position and `D_cand ≥ (1 + h)·D_inc` with `h = pool_switch_margin_bps` (25 %); otherwise it fails with `PositionsOpen` or `PoolNotDominant`. There is no pending switch, no delay and no `execute_pool_switch` for this path. Open positions therefore never see a pool change that they could not also see on a failure, and the v0.3 four-timer state (window, margin, pending switch with delay, fast-path window) shrinks to two.
   **Griefing path (disclosed; R4B-10).** One minimum-size position (0.1 SOL notional; Tier A up to 30 days, re-opened at expiry) keeps `open_positions ≥ 1` and so pins the market to an eligible but much thinner incumbent — lower caps and tier, cheaper manipulation — until that incumbent fails. No lender funds are at risk; it is a quality regression from v0.3's pending switch. Remedy without new state: Emergency pauses opens on the market (existing power, SPEC §9), the market drains within the longest open duration, and the keeper's duty 12 adopts the dominant pool at `open_positions == 0`. A delayed adoption with positions open (e.g. `D_cand ≥ 2·D_inc` for 1 h) was considered and not adopted: it re-creates the pending switch that decision 7 removed. Owner item O18 lists it.
3. **Fast path** (incumbent failure; A71; V1-7, V2-1, V2-7, R3A-4, R3B-8). The trigger is its own predicate and its own chain:
   ```
   pool_fail := !venue_executable(incumbent) ∨ φ_live(incumbent) > config.venue_fee_hard_cap_bps     (venue-side evidence only)
   chain (prologue step 8, Raydium markets): on a pool_fail observation, if pool_fail_since == 0 or
       now − last_pool_fail_obs > S3_OBS_GAP_S then pool_fail_since = now; always last_pool_fail_obs = now;
       an observation without pool_fail clears both
   ```
   The Emergency route-broken flag is **not** part of `pool_fail` (T-S3-BROKEN asserts the chain is identical with and without the flag), and the chain is not the S3 chain (which is fed by `s3_no_route`). Once `now − pool_fail_since ≥ pool_fast_route_s` (120 s), the first `propose_pool` with `D_cand ≥ pool_fast_min_bps·pool_d/10⁴` (`pool_d` = the incumbent's locked depth at its last executable observation) opens a **collection window** of `pool_fast_window_s` (60 s): during it any eligible candidate with a higher `D` replaces the one in `fp_*`, and `execute_pool_switch` after the window adopts the best candidate seen, if it is still eligible and the incumbent still fails `pool_fail`. The fast path works whether or not positions are open. Its whole duration fits inside the shortest S3 window any position can have (`pool_fast_route_s + pool_fast_window_s ≤ 540 = 600 − 60`, compiled, R3A-11 b).
4. **Every adoption** — listing, listing-window replacement, zero-position adoption, fast path — may change the venue kind between `RaydiumV4` and `RaydiumCpmm`, and:
   - `P_ema` carries over (same asset, arbitraged); `L_ema` and `L_slow` reset to `min(√(X·Y)_new, current value)`;
   - **every ring entry's `l`, `llo` and `lhi` are rewritten to the reset `L_ema` and its `(x, y)` recomputed** (A72; R3B-7 a, R3A-11 d): with carry-over, the ring median kept the old pool's liquidity and the settlement box stayed more than 1 % wide in `L` for 3–4 s after each change (ECON §10.1);
   - the current observation bucket is cleared; both route chains are cleared (the new pool is observed afresh);
   - `graduated_at` is kept (age is the token's time on an eligible pool); open positions keep all locked terms and exit on the new pool;
   - `pool_seen_at = now` **only if the market is not already inside a listing window**, so opens wait `pool_list_window_s` after an adoption outside a window while the `D` ranking settles, and a chain of proposals cannot extend a window;
   - emit `PoolSwitched`.
5. **Opens are sized on locked depth, not refused on `λ_now`** (V2-8). Every liquidity input of an open is bounded by the locked depth, which unlocked deposits cannot move: tier liquidity uses `R_eff = min(R_now, R_slow, ⌊⌊D·isqrt(P_ema)/10⁹⌋·10⁴/λ_min_bps⌋)` (§4.7), and the conservative token reserve for caps is `min(X_now, ⌊L_slow·10⁹/isqrt⁺(P_ema)⌋, ⌊D·10⁹·10⁴/(λ_min_bps·isqrt⁺(P_ema))⌋)` (§4.6; ECON §2.1). Opens require `R_eff ≥ pool_min_real_sol`. Withdrawing *all* unlocked LP leaves at least `λ_min` (50 %) of the depth a position was sized on. `λ ≥ λ_min` is still required for eligibility and adoption.

| Attack | Outcome under the rule |
|---|---|
| Fake deep pool with self-supplied liquidity, pulled after capture | Unlocked liquidity adds nothing to `D`; winning requires burning LP worth `(1 + h)` times the incumbent's locked depth, which permanently deepens the market, and it can happen only while no position is open |
| Price push on the candidate at proposal time (atomic bundle) | `√(X·Y)` is invariant under swaps; no effect |
| Thin pools used for price manipulation | Never read. The market prices, observes and executes only on the adopted pool, whose keys are stored and re-checked on every instruction |
| Mint on the wrong side, fake quote mint | The SOL side is derived from the verified mint fields; the quote must be exactly `So111…112` under Tokenkeg |
| Proposal spam | A weaker proposal cannot displace the incumbent or a better fast-path candidate; each fake pool costs 0.15 SOL [R-RAY §6.3]; adoptions inside a listing window do not extend it |
| Wash trading to raise `D` (V2-7) | Costs ≈ `2·Y·ln(1 + x)` in fees, of which ≥ `λ_min` stays with burned LP; the 5 % listing margin and the 25 % adoption margin make flips expensive; with positions open only a failure can change the pool |
| Front-running the fast path with an own pool carrying unlocked LP (V2-7) | The collection window adopts the deepest eligible candidate seen; an own pool wins only if it is the deepest, and its locked part (≥ 50 % of its LP) cannot be withdrawn afterwards |
| Emergency flag used to arm the fast path (V1-7) | Impossible: `pool_fail` does not read the flag (T-S3-BROKEN) |
| Adding unlocked liquidity to block opens (V2-8) | No effect: opens do not check `λ_now`; sizing uses locked depth |
| Unlocked LPs withdraw from the incumbent | `R_now` falls, so tier liquidity and size caps fall at once; open positions keep at least `λ_min` of the depth they were sized on (rule 5), which at capped sizes cannot reach S2 (ECON §8.6); FV follows the fast liquidity EMA (D3 literal) |

---

## 3. Instructions

### 3.0 Common structure

**Remaining-accounts convention (A6; C0-2, C0-4).** Venue accounts are passed in `remaining_accounts` as

```
ReadSet(venue) ++ TradeExtras(venue) [trading instructions] ++ Extras(venue, ix) [A41]
```

and parsed by a strict per-venue parser (§3.12). v0.3's `OpenTail` (the ProgramData accounts `open_short` read for the venue guard) is gone: the guard is the `VenueGuard` latch (A7).

- **Every slot is mandatory and key-checked** against a compile-time constant, a `Market` cache entry, or an on-chain re-derivation (creator vaults). There are no optional slots and no "None marker". A missing or wrong account reverts; it never maps to "+∞", "no route" or "absent" (C0-2).
- An account that may legitimately not exist yet (the canonical PumpSwap pool before graduation, the boost-vault ATA after the boost) is still passed at its derived key; the reader reports its state ("uninitialised", amount 0). Pricing never depends on whether the caller passed it.
- **Key check versus reader failure (A63; V1-8).** The two are different events. A *wrong key* reverts, always. A *correctly keyed* venue-state account whose contents no longer parse — a changed owner, discriminator or layout of a pool, config, fee config or global account after a venue upgrade — makes the venue **not executable** (§5.3). For `crank_price` and `sol_settle` that is a normal outcome (the prologue records a no-route observation and the S3 chain runs), never a revert; trading instructions then fail at their own route check. S3 therefore stays reachable when a venue changes a state account's layout.
- **Bounded extras (A41; C0-4 (a), V2-6).** After the adapter's own list, the parser accepts exactly the trailing accounts of the extras set the caller selects in the instruction data: `none`, `current` or `previous` from `VenueExtras[venue][ix]` (§2.3.2), 0–4 entries in order. A fixed-key entry must match; a **template** entry is re-derived on-chain from its constant seeds and placeholders (`mint`, `pool`, `trader`, `bonding_curve`, `coin_creator`) with the caller's bump, so per-mint and per-pool PDAs — the kind of account venues actually add (pump's `bonding_curve_v2`, PumpSwap's `pool-v2` [C1-10; R-VEN]) — can be followed. Each extra must be a non-signer, not owned by short.win and not on the denylist, and writable only if its flag is set; they are appended to the venue CPI's metas. This lets the Emergency multisig follow an *additive* venue account change (pump's 2026-04-28 upgrade added trailing accounts with 6 days' notice and allowed passing them early [R-PUMP Review corrections 4; C0-4]) without a program upgrade. Because `none` and the previous set stay selectable, one wrong or hostile Emergency write cannot brick exits even on a venue that rejects unexpected trailing accounts (pump behaviour **UNVERIFIED**, U24); Raydium ignores trailing accounts [R-RAY §9.6]. A *non-additive* change cannot be followed; it reaches S3 through reader failure (above) or the route-broken flag (A40), with the liveness limits of §5.3.
- Named Anchor accounts are never repeated in remaining accounts. CPI metas are assembled from both lists, so Anchor 1.x duplicate-mutable checks never see duplicates [R-TOOL §1.3].

**Market prologue.** Every instruction that loads a `Market` for writing runs these steps in order (ECON §2.1 order of operations, §5.1–5.2, §7, §9.4):

1. Load `config` (read) and `market` (mut); check `version`.
2. **Params-version sync (A17, A59; R3A-9).**
   - EMA fields (`ema_dtmax_s`, `ema_clamp_bps`, `liq_slow_tau_s`, `liq_slow_dtmax_s`): if `market.ema_params_version ≠ config.ema_params.version` and (`open_positions == 0` or `now ≥ config.ema_params.force_at`), copy them and set `ema_params_version`.
   - Kink: if `market.kink_version ≠ config.kink.version` and `open_positions == 0`, copy `util_kink_bps` and set `kink_version`. A forced EMA adoption never touches the kink or its version, so a kink change waits for the first moment the market has no open position (test: force-adopt with positions open, drain to 0 positions, assert the kink is then adopted).
   - The ring keeps its entries (they are prices, not parameters).
3. Mint guard: `mint.key == market.mint`, `*mint.owner == market.token_program == token_program.key`. Entry paths (`create_market`, `deposit`, `open_short`) require `check_mint_safety`; exit paths emit `MintSafetyTripped` and continue [R-MINT §6, §9.4].
4. **Accrue** (ECON §7, §9.4): (a) **vest release**: `rel = ⌊vest_bal·(now − vest_last)/(vest_end − vest_last)⌋` (all of it once `now ≥ vest_end`) is credited to entitlement and realised allocation together (`acc`, `acc_r`), `vest_last = now`; (b) **rent**: the three basis indices with `P̃ = min(P_ema, P_ACC_MAX)` taken **before** this instruction's roll and `U` from the pre-instruction counters; the lender credit `⌊Σ_j A_j·δJ_j/(10⁴·10⁴·31,536,000·WAD)⌋` goes to `acc` and `receivable` (§4.5).
5. **Observation roll**: if a bucket is open and `now > b_sec`: if `open_positions > 0 ∧ now − ema_ts > COVERAGE_GAP_S` emit the `CoverageGap{gap_s = now − ema_ts, open_positions}` log event (A69); integrate the bucket into `P_ema`, `L_ema`, `L_slow` and push a ring entry `(ts, p, l, x, y, lo, hi, llo, lhi)` (v0.4.1, R4B-2; §4.4): `(p, l) = (P_ema, L_ema)`, the cached `(x, y) = reprice(L_ema, P_ema)` (on the curve these are the virtual reserves `(vT_i, vQ_i)`, and `curve_reprice` applies the curve's constant offsets only when quoting, so `integrate` takes no venue argument), and `(lo, hi, llo, lhi)` the completed bucket's price and liquidity extremes (the clamped `obs`, `lobs` drive the EMA step but are not stored); move the bucket to `lb_*`; clear it (ECON §5.2).
6. If the instruction carries a ReadSet: parse it, read `VenueState`, run `sync_venue` (§2.5). Reader failures of correctly keyed accounts yield "not executable" (A63). A venue change (graduation) resets the liquidity EMAs and rewrites the ring (A72).
7. **Observation record**: if the venue is `venue_executable` (or `PumpMigrating`, whose completed curve is final) and its price is defined, add `(P_spot, √(X·Y))` of the **pre-trade** state to the bucket of second `now` (ECON §5.1). Every instruction records; there is no one-per-slot limit (C0-5 (c)). A venue that cannot execute is not observed (A64; V2-5). A fee above the hard cap does not stop observation: the pool still trades.
8. **Route chains** (§5.3): update the S3 chain with `s3_no_route`; on Raydium markets also update the fast-path chain with `pool_fail` (§2.6). Both use the **global** `venue_fee_hard_cap_bps` (R3A-4 c).
9. Instruction body. Decisions read `P_ema`, the ring and `lb_*`, which never contain this instruction's own observation (the one-second lag, ECON §5.2).
10. Post-conditions: trader state (§2.4 rule 5), `pool_vault.amount ≥ T`, vault lamport invariants (§2.3.1).
11. One `emit_cpi!` event per instruction (merging what v0.1 emitted separately), to save instruction-trace entries. `CoverageGap` is the one exception: it is an `emit!` log event (no CPI, no trace entry), because it is advisory and can coincide with any instruction; indexers also derive coverage from `PriceCranked` timestamps.

**A16: lender instructions.** `deposit` and `withdraw` run steps 1–5 and 10–11: they accrue (including the vest release) and roll the bucket, but carry no venue accounts and record no observation. `claim_sol`, `cover_deficit`, `sync_lender` and `add_margin` do not write `Market` at all (C0-17); the first three write `MarketVault` (pot, lock state) and the lender. The EMA is fed by trading instructions, `sol_settle`, `propose_pool` and `crank_price`; the in-house keeper supplies one observation per second near thresholds and counts its own `sol_settle` / `liquidate` / `expire` calls as that second's observation (ECON §5.8).

### 3.1 Initialization and admin

| Instruction | Signer | Accounts | Effect | Checks |
|---|---|---|---|---|
| `initialize_config(args)` | short.win **upgrade authority** | `config` (init), `venue_guard` (init), `venue_extras` ×4 (init, empty), `oracle_cache` (init), `program_data` (short.win's own; `upgrade_authority == signer`), system | Writes authorities, params, tiers, EMA and kink params, `timelock_s`; `venue_guard.guard_slot` from the args | Prevents front-run initialisation. `timelock_s ≥ MIN_TIMELOCK_S`. `validate_params()` (`params::validate`, then `params::validate_econ`; §2.3.3) |
| `propose_config(change)` | `governance` | `config` (mut) | `pending = { change, not_before = now + timelock_s }`; replaces any pending change | `validate_params()` (`params::validate`, then `params::validate_econ`; §2.3.3) on the would-be result. Off-chain gate (A73): the proposal's overrides pass `sim/config_check.py` (`OK`) before the multisig signs |
| `apply_config()` | anyone | `config` (mut) | Applies `pending`, clears it | `now ≥ not_before`. Variants: `Params(Params, [TierParams;4])`, `EmaParams{…}` (new `version`, `force_at`), `Kink{util_kink_bps}` (new kink `version`), `Authorities{governance, pause_authority, coverage_authority, fee_recipient}`, `Timelock(u32)` (≥ MIN), `EnableVenues(u32)`, `OracleFeed{account, feed_id}`. **There is no variant that changes a venue program id or a venue PDA (C0-8)** |
| `cancel_config()` | `governance` or `pause_authority` | `config` (mut) | Clears `pending` | |
| `set_pause(scope, paused)` | `pause_authority` or `governance` | `config` or `market` (mut) | Sets or clears `paused_opens` | SPEC §11: new opens **only** |
| `set_coverage_pause(paused, reason)` ★ | `coverage_authority` (A69; O17) | `config` (read), `market` (mut) | Sets or clears the market's `paused_coverage` flag (reason code `KeeperCoverage`); emits `CoveragePauseSet` | Inert while `coverage_authority == Pubkey::default()` (the default until O17). Opens only; moves nothing |
| `disable_venue(bit)` | `pause_authority` | `config` (mut) | Clears a `venue_enabled` bit; re-enabling is timelocked | Opens only |
| `flag_venue_upgraded(venue)` ★ | **anyone** | `venue_guard` (mut), that program's `ProgramData` | If `programdata.slot ≠ guard_slot[venue]` and the bit is clear: set bit `venue` in `upgraded`, record `flagged_slot`, `flagged_at`, emit `VenueUpgradeFlagged`. If the bit is set and `programdata.slot ≠ flagged_slot[venue]` (another upgrade during review, v0.4.1): update `flagged_slot`, `flagged_at`, emit `VenueUpgradeFlagged` again. Otherwise fail with `VenueNotUpgraded` (no write) | Address == the compile-time ProgramData constant, owner is the upgradeable loader, tag == 3. Replaces v0.3's per-open ProgramData read and the `VenueSlots` mirror (A7; decision 4) |
| `revalidate_venue(venue, expected_slot: u64)` ★★ | `pause_authority` | `venue_guard` (mut), that program's `ProgramData` | Requires `programdata.slot == expected_slot` (the deploy slot the Emergency signers reviewed), else `VenueSlotMismatch`; then `guard_slot[venue] = expected_slot`; clear bit `venue` | Same address and owner checks. Emits `VenueRevalidated{old, new}`. The expected slot stops an upgrade that lands while the Squads proposal collects signatures from being validated silently (R4B-7) |
| `set_route_broken(scope, broken)` | `pause_authority` or `governance` | `config` (venue bit) or `market` (mut) | Sets/clears `venue_route_broken` or `market.route_broken` (A40, C0-4 (b)) | **Built only after O6 is recorded in SPEC §14** (R3A-8 b). Moves no funds. Its only effects: opens refused, and `s3_no_route` for the S3 chain (§5.3). It is **not** an input to `venue_executable`, to any quote or leg, to close / liquidate / expire, or to `pool_fail` (V2-1, V1-7); T-S3-BROKEN proves it |
| `set_venue_extras(venue, ix, entries)` | `pause_authority` or `governance` | `venue_extras` (mut) | Moves the `current` set to `previous`, writes `entries` (≤ 4 fixed keys or derivation templates) as `current` (A41, C0-4 (a), V2-6) | **Built only after O6.** Fixed keys and template programs may not be short.win or its PDAs; at use time each extra must be non-signer and not owned by short.win. Callers may always select `none` or `previous` |
| `sweep_protocol_fees()` | anyone | `config`, `market`, `market_vault` (mut), `fee_recipient` (mut, == config) | Moves `vault.protocol_fees` lamports to `fee_recipient`; sets it to 0 | The only debit of protocol money |

**What admins cannot do** (SPEC §9–§11; I4): move tokens out of `pool_vault`, move lamports out of `ShortPosition` or `MarketVault` (except the fee sweep to the configured recipient), change a position's locked terms, change a venue program id or venue PDA, re-point a market's mint, vaults or pool (pool changes only through the permissionless rule of §2.6), pause closes, liquidations, settlements, withdrawals or claims, or set any parameter outside its compiled bounds.

**Venue-upgrade latch (A7 as amended; ADOPTED Q13; masks v0.4.1).** Opens are refused while `venue_guard.upgraded` has a bit of the market's venue mask set: PumpCurve: pump + pfee + **pAMM** (a curve position exits on PumpSwap after graduation, and the straddle quote `B̂_str` prices the pAMM pool); PumpSwap: pAMM + pfee; RaydiumV4 and RaydiumCpmm: **AMM v4 + CPMM** (a fast path can move the market from one to the other while positions are open) (R4B-7). The keeper subscribes to the six ProgramData accounts over Yellowstone and calls `flag_venue_upgraded` in the slot after it sees a change (§7.1); anyone else can too. The guarantee is therefore the v0.3 mirror's (opens stop within a watcher's reaction time after an upgrade, which takes effect one slot after deployment), with no per-slot writes and no 10 MiB ProgramData copy per open (C1-15, V1-11). The residual — an open that lands between the upgrade taking effect and the flag — is disclosed (§8.5).

**New Emergency powers (A40, A41; owner item O6)** move no funds and are disclosed. They are not "pause" or "fee parameters" in SPEC §9's sense, so they are built only after the owner records them as a SPEC §9 amendment (R3A-8 b); until then S3 on an interface break relies on reader failures (A63). What they can do, stated fully (V1-7): the route-broken flag refuses opens and starts the S3 chain, which after `s3_wait_s` lets *liquidatable or expired* positions settle in SOL at `min(V − K_s, FV)` (checked on the settlement box) instead of waiting for a buyback; the extras sets change the trailing accounts of venue CPIs, and a bad set degrades nothing because callers can pick `none` or the previous set.

The alternative the review offered, a short-timelock upgrade path for adapter patches (C0-4 (c)), is **not** adopted. Corrected rationale (V1-8): the 48 h Governance upgrade authority already exists and is disclosed (§8.5), so a second, faster path would not add a new *kind* of power; it would shorten the window in which users can react to an arbitrary code change from 48 h to hours. Adapter-level changes that can be data (additive accounts, including per-mint PDAs) are covered by the extras templates; everything else reaches S3 or waits for the 48 h upgrade.

The program upgrade authority (the Governance multisig, 48 h time_lock) can still replace the code; that is disclosed as a trust assumption (§8.5).

### 3.2 `create_market(venue_hint: u8)`

**Signer:** `creator` (payer).

**Named accounts:** `config`; `market` (init, `["market", mint]`); `market_vault` (init); `pool_vault` (init, `token::mint = mint, token::authority = market, token::token_program = token_program`); `mint` (InterfaceAccount<Mint>); `trader` (Unchecked, seeds); `trader_base_ata` (Unchecked; created by CPI `create_idempotent` for pump venues, then validated); `token_program` (Interface); `associated_token_program`, `system_program`; `event_authority`, `program`.

**Remaining:** always the detection set `[bonding_curve PDA, canonical_pool PDA]` (both derived on-chain and key-checked, C0-15); then the init set of the detected venue:

- PumpCurve: pump program, pump UVA, pump event authority (for `init_user_volume_accumulator`);
- PumpSwap: pAMM program, pAMM UVA, pAMM event authority;
- `venue_hint = RaydiumV4 / RaydiumCpmm`: the candidate's ReadSet (§3.12), which must pass `eligible()` (§2.6).

**Checks.**

1. `check_mint_safety(mint)`, a hard failure (ADOPTED Q7) [R-MINT §9.2]: owner ∈ {Tokenkeg, Token-2022}; not a native mint (`So111…112`, `9pan9bMn…`); `freeze_authority == None`; every TLV type ∈ {MetadataPointer, TokenMetadata, GroupPointer, TokenGroup, GroupMemberPointer, TokenGroupMember}; any parse error rejects.
2. `token_program.key == mint.owner`.
3. Venue detection (§2.5), with the D1 precedence and the ADOPTED Q6 rejections (mayhem mode, non-SOL quote). Curve accounts shorter than 115 bytes fail with `CurveNeedsExtend` (the client prepends pump `extend_account`) [R-PUMP §2.1]. PumpSwap pool accounts shorter than 300 bytes are allowed; readers zero-pad and clients prepend `extend_account` before trading [R-VEN §1.2].

**Effects.**

- Initialise `Market`: identity and bumps; own keys; `bonding_curve` and `canonical_pool` keys; the venue caches (curve ATAs with the **mint's token program** for base and Tokenkeg for WSOL [R-MINT §8.5]; `sharing_config = PDA(pfee, ["sharing-config", mint])`; UVA PDAs; creator vault from `curve.creator`; or the Raydium pool keys and SOL side); `venue`, `created_at = now`.
- Price state if the venue has a price: `P_ema = P_spot`, `L_ema = L_slow = √(X·Y)`, `ema_ts = slow_ts = now`, a ring with the one entry `(now, P, L, P, L, reprice(L, P))` and an empty bucket (ECON §5.2); `share_epoch = 0`, `MarketVault.pi = WAD`, basis indices 0, `ema_params_version` and `kink_version` current.
- Age (A12, ADOPTED Q18; O13 extended by R3B-12): PumpSwap `graduated_at = now` (age from listing, conservative for a pool that graduated long ago); Raydium CPMM `graduated_at = min(now, max(open_time, T_CLAMP))` (a valid lower bound on pool age [R-RAY §7]); Raydium AMM v4 `graduated_at = now` (`pool_open_time` is unusable [R-RAY §7]); Raydium markets set `pool_seen_at = now` (opens wait `pool_list_window_s`).
- Pump venues: CPI `ATA::create_idempotent(trader_base_ata)` (payer creator), then validate mint, owner and program; CPI `init_user_volume_accumulator(payer = creator, user = trader)` on the current venue's program, skipped if the UVA already exists (a griefer creating it first is harmless: it is keyed to `trader`) [R-PUMP §3.6].
- `tier` computed (§4.7).
- System-transfer `market_creation_fee` from the creator to `market_vault`; `vault.protocol_fees += fee`.

**Event:** `MarketCreated{…, fee}`. **Errors:** `MintUnsupportedProgram`, `MintNative`, `MintFreezeAuthority`, `MintDisallowedExtension`, `MintMalformed`, `CurveMayhemUnsupported`, `CurveQuoteNotSol`, `CurveNeedsExtend`, `VenueAccountMismatch`, `PumpVenuePrecedence`, `PoolIneligible`.

Listing is permissionless and never needs a venue: a `None` market accepts deposits and refuses opens until a venue appears (§2.5). Cost to the creator: ≈ 0.0297 SOL of account rent on Raydium (`Market` 3,104 B: 16,418,560 lamports; `MarketVault` 2,200 B: 11,826,240; `pool_vault` 1,488,440) and ≈ 0.0326 SOL on pump venues (plus the trader ATA and UVA), plus the 0.005 SOL fee: ≈ 0.035–0.038 SOL in total (v0.4: 0.023–0.026; the difference is v0.4.1's 2 KB lock expiry calendar and the wider ring entries; SPEC §3 suggests ~0.02 SOL; ADOPTED Q24). A weekly calendar (16 buckets) would save ≈ 0.009 SOL per market but lengthen a lock by up to 7 days instead of 1. Rent-exempt minimum = `(len + 128)·5,080` lamports [V5].

### 3.3 Lender instructions

These implement `sim/pool_model.py` v0.4.1 exactly (`advance`/`_release`, `_vest`, `_credit`, `_credit_r`, `_cal_advance`, `_cal_add`, `_cal_sub`, `lock_until_for`, `_touch`, `_to_pot`, `_maybe_roll`, `_roll`, `_resolve`, `_lock_of`, `locked_total`, `free_allowance`, `deposit`, `withdraw`, `claim_sol`, `cover`, `sweep_sol`, `borrow`, `repay`, `settle`; ECON §9). Option (a) of the round-3 brief is the design (continuous rent entitlement, SPEC §2.4); option (b) is owner item O16 (§10). Where the state lives: `acc`, `acc_r`, `S`, `T`, `ΣB_Q`, `receivable`, `max_release`, the vest, the share epoch and the post-wipe sweep bits on `Market`; `lender_sol`, the pot `unmatched`, `protocol_fees`, the lock state (Π, `pi_epoch`, `lock_norm`, `b_min`) and the lock expiry calendar on `MarketVault` (§2.3.6).

**`credit(x, realised)`** (shared, pool_model `_credit`): if `S > 0`, `acc += ⌊(x·WAD + carry)/S⌋` (remainder in `carry`) and, if realised, likewise `acc_r`. If `S == 0`: if `wipe_live`, add `x·WAD/s_old` to the live full-wipe record (`acc_end`, and `acc_r_end` if realised; remainders in the record's carries; V2-12); **otherwise the SOL has no lender and goes to the protocol**: `lender_sol −= x`, `protocol_fees += x` (R3A-10; `unassigned_sol` is gone).

**`vest(x)`** (pool_model `_vest`; R3B-2, R4B-5): lumps — the lender half of a penalty, the expiry fee, the part of an S2/S3 take above `⌊Q·P_ema/WAD⌋ + R_due` (R4B-6), trader sweeps and venue cashback — are already in `lender_sol` and are released linearly: release what is due first; if `S == 0`, `credit(x, realised)`; else, if `vest_bal > 0 ∧ vest_end > now`, `vest_end = now + ⌈(vest_bal·(vest_end − now) + x·lump_vest_s)/(vest_bal + x)⌉` (value-weighted: a dust lump moves the end at most 1 s later, and no lump moves it earlier while `lump_vest_s ≥ vest_end − now`, i.e. unless governance lowered `lump_vest_s` below the remaining span), otherwise `vest_end = now + lump_vest_s`; then `vest_bal += x`, `vest_last = now`. v0.4 restarted the window for the whole balance on every lump, which 1-lamport sweeps could exploit (ECON §9.4). S1 SOL and the immediate part of an S2/S3 take are **not** vested.

**`touch(lender)`** (shared, pool_model `_touch`): for each share epoch the lender is behind, read the `ShareEpoch` record (remaining accounts, key-checked PDAs `["share_epoch", market, e]`), then `pending += shares·(acc_end − snap)/WAD`, `real += shares·(acc_r_end − snap_r)/WAD`, and:

- **Live wipe record (V2-12):** if that record is the live full wipe (`wipe_live`, `num = 0`), the lender stays on it (`snap = acc_end`, `snap_r = acc_r_end`), moves any surplus to the pot, and stops: it has no current shares and `withdraw` refuses.
- otherwise `shares = ⌊shares·num/den⌋`, `lock = ⌊lock·num/den⌋`, `snap = snap_r = 0`, `epoch += 1`; **jump (V2-12):** if that left `shares == 0 ∧ lock == 0`, set `epoch = market.share_epoch`, `snap = acc`, `snap_r = acc_r` at once.

Then `pending += ⌊shares·(acc − snap)/WAD⌋`, `real += ⌊shares·(acc_r − snap_r)/WAD⌋`, `snap = acc`, `snap_r = acc_r`, and:

- **to the pot (R3A-2):** `surplus = (real − claimed) − max(pending, 0)`; if `surplus > 0`, `real −= surplus` and `MarketVault.unmatched += surplus`. Realised SOL is allocated to the shares holding when it arrives, entitlement to the shares that held while it accrued; the surplus of a lender who joined late is exactly the deficit of one who left early, and the pot connects them without a division by `S − shares` and without a keeper duty.
- **lock expiry (R3B-1; v0.4.1):** if `lock > 0 ∧ now ≥ lock_until ∧ lock_epoch == pi_epoch`: `lock = lock_term = 0`; `lock_epoch = NONE`. The aggregate is **not** touched here: the calendar dropped the lock's bucket at `lock_until` (§2.3.6), so every other lender's allowance stopped counting it at that second, whether or not this lender is ever touched (R4A-2, R4B-3). The epoch conversion scales `lock_term` along with `lock`.

If a lender is behind by more epochs than fit in one transaction, `sync_lender` advances it first.

**`deposit(amount: u64, min_shares: u128)`**

- Signer `owner`. Accounts: `config`, `market` (mut), `market_vault` (mut), `lender` (init_if_needed, `["lender", market, owner]`), `mint`, `owner_token_account` (mut; mint and token program checked), `pool_vault` (mut), `token_program`, `system_program`, event accounts; `ShareEpoch` records if behind.
- Checks: mint guard (Entry); `amount > 0`; `minted > 0`; `minted ≥ min_shares`; `S + minted ≤ u128::MAX`.
- Effects: prologue steps 1–5 (vest release included); `touch`; `minted = ⌊amount·(S + VS)/(T + ΣB_Q + VA)⌋`; if `S == 0 ∧ wipe_live`: **require `wipe_trader_swept` and — on a pump venue that can execute — `wipe_rewards_claimed`, else `WipeSweepPending`** (R4A-7; both instructions are permissionless, so a depositor bundles them, and they credit the wipe record while `S == 0`), then clear `wipe_live` (the wipe record is final from now on) and `touch` again; `transfer_checked(owner → pool_vault)`; `T += amount`, `S += minted`, `shares += minted`.
- CPI Guard: a lender whose token account has `CpiGuard.lock_cpi` cannot deposit through our CPI; the frontend detects it. A delegate-based deposit path is v1.1 [R-MINT §8.4].

**`withdraw(shares: u128, min_tokens: u64)`**

- Signer `owner`. Accounts: as `deposit`.
- Checks (C0-9, C0-13): mint guard (Exit: never reverts on the filter); `pending ≥ 0` after `touch`, else `NegativePending` (use `cover_deficit`); the lender is on the current epoch; `shares ≤ lender.shares`; `tokens = ⌊shares·(T + ΣB_Q + VA)/(S + VS)⌋ ≤ T` and `≤ allowance`, with (ECON §9.3; `pool_model.free_allowance`)
  ```
  cal_advance(now)                                                           (§2.3.6; v0.4.1)
  lock_i(now) = 0 if now ≥ lock_until or lock_epoch ≠ pi_epoch, else min(⌈lock·Π/lock_pi⌉, shares_i)
  locked      = min(⌊lock_norm·Π/WAD⌋, S) ;  u = shares_i − lock_i(now)      (floor, v0.4.1)
  allowance   = min(⌊u·T/max(S − locked, u)⌋, ⌊shares_i·(T + ΣB_Q + VA)/(S + VS)⌋, T)
  ```
  and `tokens ≥ min_tokens`.
- **Lock update**: `b_max = ⌈allowance·(S + VS)/(T + ΣB_Q + VA)⌉`; if `ΣB_Q > 0 ∧ b_max > 0 ∧ u > b_max`, `add = ⌈min(shares, b_max)·(u − b_max)/b_max⌉`, else `add = 0`; `new_lock = min(lock_i(now) + add, shares_i − shares)`; if `new_lock > 0 ∧ lock_norm == 0`, `b_min = ΣB_Q` (first lock of the Π epoch, V2-3); **if the current lock is live** (`lock_i(now) > 0`), remove its old term from its bucket `lock_until/86,400` and from `lock_norm` (an expired lock's bucket is already gone); `lock_term = ⌊new_lock·WAD/Π⌋` (floor, v0.4.1); **`lock_until = max(lock_i(now) > 0 ? lock_until : 0, ⌈(max(now, market.max_release) + lock_grace_s)/86,400⌉·86,400)` when `add > 0`** (unchanged otherwise); add `lock_term` to bucket `lock_until/86,400` and to `lock_norm`; `lock_pi = Π`, `lock_epoch = pi_epoch`.
- **Last lender (R3B-2, R3A-10):** if `shares == S` and `vest_bal > 0`, release the whole vest to the current shares first and `touch` again, so the leaving lender is paid it. **Any other exit forfeits the lender's unreleased vest to the lenders who stay** — to a dust holder if only dust stays (disclosed; R4B-4). A threshold flush ("release the vest before a withdrawal that leaves < 1 % of `S`") was rejected: it lets a whale that deposits 199× just before a lump take 99.5 % of it (ECON §9.4). The frontend warns before such an exit.
- Effects: `transfer_checked(pool_vault → owner, tokens)` with the market PDA signing; `S −= shares`, `T −= tokens`; then pay SOL exactly as `claim_sol`. This is SPEC §2.4's "pro-rata mix of tokens and SOL".
- **Lock decay** (A37 as amended by A57, A75): `pool::resolve(q)` (every close, liquidation, expiry or settlement of `q` borrowed tokens) runs `cal_advance(now)`, then with `B' = ΣB_Q − q`: if `B' = 0` the Π epoch rolls (`pi_epoch += 1`, `Π = WAD`, `lock_norm = b_min = 0`, every calendar bucket cleared: all locks clear); else if `lock_norm > 0 ∧ B' < b_min`, `Π ← ⌈Π·B'/b_min⌉`, `b_min = B'`. **And every lock expires at its `lock_until`** — `max_release` is monotone and contains every open position's deadline plus its own S3 tail, so no position open at lock time can outlive it (R3B-1; R4A-8, R4B-8 — v0.4 used the deadline alone, and a position stuck in `PumpMigrating` could outlive its lock by up to 3 days at compiled-maximum timers) — which bounds a lock even in a market whose borrowing never falls. Π is kept because when interest fades it returns 94 % of a lender's allowance within 24 h, against 0 % for 31 days with `lock_until` alone (ECON §9.3).
- Close `lender` when `shares == 0 ∧ pending == 0 ∧ lock == 0`.
- Memo-required destinations: detect `MemoTransfer` and CPI the memo program first (**UNVERIFIED** that a CPI'd memo satisfies the sibling check; T-MEMO) [R-MINT §8.4].
- Withdrawals never need a venue, so they work in every venue state, including `PumpMigrating` and a broken route. No cooldown (ADOPTED Q22).

**`claim_sol()`** (C0-13, C0-17, V2-2, R3A-2)

- Signer `owner` (mut). Accounts: `market` (**read-only**), `lender` (mut), `market_vault` (mut); `ShareEpoch` records if behind.
- It does not accrue (that would write `Market`), so it pays against the last accrued `acc`, `acc_r` and vest release. It runs the full `touch` (epochs, pending/real, surplus to the pot, the lender's own lock expiry), which writes only the lender and `MarketVault`.
- **Pay** (pool_model `claim_sol`): `own = real − claimed`; `draw = min(unmatched, max(0, pending − own))`; `unmatched −= draw`, `real += draw`; `pay = min(pending, real − claimed, lender_sol)`; if `pay > 0`: `pending −= pay`, `claimed += pay`, `lender_sol −= pay`, move the lamports. A second call in the same state pays 0 (memoryful, V2-2), and no lender can draw more than its own entitlement (I-P5). First-come priority among deficit lenders on the pot is accepted (ECON §9.4).
- **Liveness (I-P6):** after every position resolves, the vest is released and every lender is synced, sequential claims pay out `lender_sol − dust` (rounding plus rebase-orphan credits, ECON §9.5). A surplus reaches the pot at its holder's next touch; `sync_lender` is permissionless for holders who never return (the frontend prepends it when the pot cannot cover a claim; §7.1).

**`cover_deficit(lamports: u64)`** (C0-13): signer `owner`; `market` (read), `lender` (mut), `market_vault` (mut). `touch`; System-transfer `lamports` to `market_vault`; `lender_sol += lamports`; `pending += lamports`; `real += lamports` (pool_model `cover`).

**`sync_lender(max_epochs: u8)`** (C0-6): anyone; `market` (read), `market_vault` (mut), `lender` (mut), up to `max_epochs` `ShareEpoch` records. Runs `touch` for up to `max_epochs` epochs: value moves only into that lender's own `pending`/`real`, or its surplus into the pot, and an expired lock's fields are cleared (the aggregate already dropped it, v0.4.1). It no longer write-locks `Market` (v0.3 recycled into `acc_r`).

**`sweep_trader` / `claim_venue_rewards`** (§3.9) credit through `vest`: with `S > 0` the SOL vests over `lump_vest_s`; while `S == 0` it tops up the live wipe record (these instructions then take the latest `ShareEpoch` record, key-checked) and sets `wipe_trader_swept` / `wipe_rewards_claimed` (even when there was nothing to move), or, with no live wipe, goes to `protocol_fees` (R3A-10). A full wipe clears both bits; `deposit` waits for them (R4A-7). The keeper still calls both right after a full wipe (ECON §5.8 duty 6).

**Borrower-side pool calls** (from §3.4–§3.8): `borrow(q, release_at, p)` releases the vest, `T −= q`, `ΣB_Q += q`, `A_j += q·coef_j·(10⁴ − p)`, `max_release = max(max_release, release_at)` with `release_at = deadline + max_migration_pause_s + grad_grace_s + s3_wait_s` (the position's snapshot values, v0.4.1); `repay(rent_paid, lump)` (close, liquidation, expiry) releases the vest, removes the position (`A_j`, `resolve(q)`, `receivable −= min(credited_for(pos), receivable)`), returns `q` to `T`, adds `lender_rent + lump` to `lender_sol` with `lender_rent = rent_paid − ⌊rent_paid·p/10⁴⌋`, credits the entitlement difference `lender_rent − credited_for(pos)` (signed; a negative value is a shortfall debit, ECON §9.4), allocates `lender_rent` as realised (`acc_r`) and vests `lump`; `settle(sol_in, lump)` does the same with `sol_in` for the extinguished tokens (principal `sol_in − lump` immediate, `lump` vested; for S2/S3 `lump = sol_in − min(sol_in, ⌊Q·P_ema/WAD⌋ + R_due)`, for S1 0) and then `maybe_roll`: if `T + ΣB_Q = 0 ∧ S > 0`, the unreleased vest is flushed into `acc`/`acc_r` and the epoch rolls with `(0, 1)` (the wiped lenders keep their lumps); if `S + VS > R_MAX·(T + ΣB_Q + VA)` it rebases with `(1, 1,000^k)` (C0-6).

### 3.4 `open_short(nonce: u64, q: u64, min_proceeds: u64, max_margin: u64, max_open_fee: u64)`

**Signer:** `borrower` (payer).

**Named accounts:** `config`, `venue_guard`, `oracle_cache` (read); `market` (mut); `position` (init, seeds with `nonce`; the client passes the bump); `exposure` (init_if_needed); `mint`, `pool_vault` (mut); `market_vault` (mut); `trader` (mut); `token_program`, `system_program`; event accounts.

**Remaining:** `ReadSet ++ TradeExtras ++ Extras` for the market's venue (§3.12). No ProgramData accounts (A7).

**Steps.** Every step is required; any failure reverts.

1. Static guards: `!paused_opens` (global and market), `!paused_coverage` (market, A69), `!mint_unsafe`, `!route_broken` (market and venue bit); `venue ∈ {PumpCurve, PumpSwap, RaydiumV4, RaydiumCpmm}` and enabled; mint guard (Entry).
2. Prologue (§3.0). The observation recorded is the pre-trade state; the EMA the body reads is the post-roll state.
3. Venue guards (ADOPTED Q13, A7, A24, A43):
   - **venue guard**: `venue_guard.upgraded & mask(venue) == 0`, else `VenueUpgraded` (masks, v0.4.1: PumpCurve pump + pfee + pAMM; PumpSwap pAMM + pfee; RaydiumV4 and RaydiumCpmm AMM v4 + CPMM; R4B-7);
   - `!s3_no_route` (§5.3: the venue is executable, no route-broken flag, fee within the global hard cap);
   - one-way venue fee `φ_ppm ≤ venue_fee_cap_bps·100`;
   - PumpSwap: `disable_flags == 0` (any non-zero value blocks opens and alerts, A24);
   - Raydium: `now ≥ pool_seen_at + pool_list_window_s` and `R_eff ≥ pool_min_real_sol` (§2.6 rule 5).
4. Open guards (ECON §4): `now ≥ created_at + market_warmup_s`; `|P_spot/P_ema − 1| ≤ open_ema_dev_bps`; `P_ema ≤ P_OPEN_MAX` (else `PriceAboveOpenMax`); PumpSwap boost gate (A13, ADOPTED Q15; C1-2): refuse while `boost_vault.amount > boost_dust_lamports ∧ now < graduated_at + boost_window_s` (the boost-vault ATA is passed at its derived key; a missing account reads as 0).
5. Tier: `tier = tier::assign(…)` (§4.7), cached on `market`.
6. Caps (§4.6), with `X₀` the conservative pre-trade token reserve and `X₁` the post-open one: SPEC per-position and per-market impact caps, the fee-scaled caps, utilisation after the open ≤ `util_cap_bps`, Tier D `ΣQ + q ≤ γ·(rT + q)`, `exposure.q_open + q ≤ wallet_cap_bps · OI_cap / 10⁴`, `q ≤ T`.
7. Expected proceeds `S_exp = quote_sell(q)` on the live state (PumpSwap enforces the real-reserve cap).
8. **Sell leg** (§2.4.1) with `min_out = max(min_proceeds, ⌊S_exp·(10⁴ − exec_tol_bps)/10⁴⌋)`. Measure `S`.
9. Reload the venue state. `N_open = B̂_spot(q)` on the post-open reserves; `B̂_ema(q) = B̂(q; x, y)` on the **newest ring entry's cached reprojection** — the EMA reserves as they stand after this instruction's roll (D4 literal; R3A-12). Require `N_open ≥ min_notional`; `total_notional_open + N_open ≤ market_max_notional`.
10. **Margin (D4):** `M = max(0, ⌈cr_open_bps·max(B̂_spot, B̂_ema)/10⁴⌉ − S)`; when `B̂_ema` is undefined (for example `q ≥ x`, the cached token reserve), the margin prices `B̂_spot` alone — the one place an undefined quote does not read as `+∞` (§4.3; `risk::margin_d4`, the normative `shortwin_sim.Market.open`: `B_margin = max(qs, qe or qs)`). Require `M ≤ max_margin`. System-transfer `M` from the borrower to `position`.
11. Open fee: `fee_open = ⌈open_fee_bps·N_open/10⁴⌉ ≤ max_open_fee`; System-transfer it from the borrower to `market_vault`; `vault.protocol_fees += fee_open`.
12. Write the position: `q, s = S, m = M, v = S + M, n_open, fee_open, entry_price = ⌊S·WAD/q⌋`, `j_open = rent_basis`, `r_min` (ECON §2.1: `⌈Q·P_ema·(b·10⁴ + s1·min(U, k) + s2·max(U − k, 0))·min_rent_s/(10⁴·10⁴·31,536,000·WAD)⌉`), `t_open = now`, `deadline = now + max_duration_s`, every locked term of §2.3.5 from the current config and tier (including `rent_protocol_bps` and `delta_liq_bps`; the effective `ert` is 0 on Raydium), arming state 0.
13. Market: `pool::borrow(q, release_at, rent_protocol_bps)` (`T −= q`, `ΣB_Q += q` and by tier, `A_j += q·coef_j·(10⁴ − p)`, `max_release = max(max_release, deadline + max_migration_pause_s + grad_grace_s + s3_wait_s)` with the values snapshot into this position, v0.4.1), `open_positions += 1`, `total_notional_open += N_open`; `exposure.q_open += q`. Post-condition: utilisation ≤ cap (SPEC §10).
14. Event `Open{…, fee_open}`.

**Errors:** `OpensPaused`, `CoveragePaused`, `VenueDisabled`, `VenueUpgraded`, `VenueFlagged`, `RouteBroken`, `NoSupportedVenue`, `VenueMigrating`, `VenueFeeTooHigh`, `PoolListWindow`, `PoolTooThin`, `MarketWarmingUp`, `PriceDeviatesFromEma`, `PriceAboveOpenMax`, `BoostInProgress`, `CapPositionImpact`, `CapMarketOi`, `CapUtilization`, `CapCurveGamma`, `CapWallet`, `InsufficientInventory`, `BelowMinNotional`, `MarketNotionalCap`, `ProceedsBelowMin`, `MarginAboveMax`, `OpenFeeAboveMax`, `VenueResultMismatch`, `VenueRentOnSell`, `TraderStateInvalid`, `DelegateNotCleared`.

### 3.5 `close_short(max_buyback_cost: u64)` and `add_margin(amount: u64)`

**`close_short`**

- Signer: `borrower` (== `position.borrower`).
- Named accounts: as `open_short` without `venue_guard` and `oracle_cache`; `position` is mut with `close = borrower`, `exposure` is mut.
- Remaining: `ReadSet ++ TradeExtras ++ Extras`. Exits never check the venue guard, pause flags, the route-broken flag or the oracle (I10).

Steps:

1. Prologue; mint guard (Exit).
2. `venue ∉ {PumpMigrating, None}` and `venue_executable`, else `VenueMigrating` / `NoRoute` (SPEC §5.3). The route-broken flag is not checked (I10, V2-1). On a live curve with `q > rT`, fail with `CurveStraddle`; the client uses the bundle of §5.4.
3. `R = rent::due(position, rent_basis)` (§4.5).
4. `B̂ = B̂_live(q)` at the **live** venue fee (what the venue will charge). Require `B̂ + R + ert ≤ V`, else `InsufficientVault` (add margin or wait for liquidation).
5. **Buy leg** (§2.4.1) with `max_in = min(max_buyback_cost, V − R − ert)` (C0-14). Require `B̂ ≤ max_in`. Assert `B ≤ max_in + ert` and at least `q` tokens into `pool_vault` (surplus: donation, V2-10).
6. Waterfall `close` (§5.2): `B` (spent) → `R` (`⌊R·rent_protocol_bps_pos/10⁴⌋` to protocol, the rest to lenders) → the remainder stays on `position`, which is closed to the borrower (A18).
7. Market: `pool::repay(R, 0)` (§3.3: token return, rent true-up, `resolve(q)`), `open_positions −= 1`, `total_notional_open −= n_open`; `exposure.q_open −= q` (closed to the borrower at 0). If the buy completed the curve (`q == rT`), `sync_venue` moves the market to `PumpMigrating` after the leg.
8. Event `Close`.

Closing while liquidatable is allowed with no penalty (A25, ECON G28). A borrower may close even when the venue fee exceeds `venue_fee_hard_cap_bps`, if its own `max_buyback_cost` allows it: closes use the venue's live fee and `venue_executable`, never the hard cap or the flag (V2-1).

**`add_margin`** (A20, ADOPTED Q12): signer `borrower`; `position` (mut), `market` (read), system program. System-transfer `amount` to `position`; `v += amount`, `m += amount`. It only raises CR. Event `MarginAdded`.

### 3.6 `liquidate()`

- **Signer:** `keeper` (anyone; receives the keeper share). Permissionless (SPEC §5.1).
- **Named accounts:** as `close_short`, plus `borrower` (mut, == `position.borrower`; receives the position close, A18).
- **Remaining:** `ReadSet ++ TradeExtras ++ Extras`.

Preconditions, evaluated on-chain in this order (ECON §5.5, §8.2, §8.3):

1. Venue is not `PumpMigrating` or `None` (SPEC §5.3 pause) and is `venue_executable`, else `NoRoute`; the route-broken flag is not read (I10). Live curve with `q > rT`: `CurveStraddle` (bundle, §5.4).
2. A fresh completed bucket exists (`lb_sec ≥ now − ref_max_age_s`, the position's locked value), else `ReferenceStale` (the keeper cranks one second and acts the next, ECON §5.4).
3. `liquidatable := (spot leg ∧ EMA leg) ∨ now ≥ deadline` (§4.5): the EMA leg reads the **newest `liq_persist_n` ring entries' cached `(x, y)`** (decision 2); risk quotes use the fee `min(φ_live, venue_fee_hard_cap_bps_pos)` (V2-1).
4. Normal buyback allowed: `B̂_live + ert ≤ V − K_s ∧ σ_live ≤ σ_max` (here `B̂_live` at the live fee, since it is the execution cost); otherwise `UseSolSettle`.
5. If `now ≥ deadline` and **not** CR-liquidatable: `UseExpire`. Expired **and** CR-liquidatable means the penalty applies (ECON §8.4).
6. **Reference bound** (C0-1; ECON §8.2 v0.4.1), with the position's locked terms and ECON §2.1's integer forms:
   ```
   g       = max(0, min over the last kappa_growth_n ring steps of ⌊(P_i − P_{i−1})·10⁴/(P_{i−1}·Δt_i)⌋)   [bps/s]
   κ_eff   = min(kappa_liq_bps, kappa_base_bps + (EMA_TAU_S + 1)·g)
   lo_x    = ⌊lb_lo·(10⁴ + g·(now − lb_sec))/10⁴⌋
   P_ref   = min( max(P_ema, lo_x), ⌊P_ema·(10⁴ + κ_eff)/10⁴⌋, ⌊lo_x·(10⁴ + κ_eff)/10⁴⌋ )      (last term v0.4.1, R4B-2)
   L_b     = max(L_live, L_ema, L_slow)                                (quiet-second LP withdrawals cannot lower it; V2-14)
   B̂_ref   = B̂(Q; reprice(L_b, P_ref))                                 (risk quote)
   max_in  = min( ⌊B̂_ref·(10⁴ + delta_liq_bps)/10⁴⌋,  V − K_s − ert )                  (v0.4.1: no reserve; SPEC §5.2)
   ```
   `δ = delta_liq_bps` is **fixed** (2 %; decision 1). v0.3's Dutch clock widened it 1 %/s over "executable" seconds; the reviewer's liveness runs and ECON's show a fixed 2 % gives the same outcome in every genuine-move cell, while a fixed 1 % turns three drain-then-pump liquidations into S1 (ECON §12.2), and blocking the spot leg can no longer widen the bound (V1-1 is closed as a class). The `lo_x·(1 + κ_eff)` term keeps a `P_ema` that one attacker-only second pushed and later seconds froze (ECON §5.6) from lifting the reference: a frozen EMA has no sustained growth, so the reference stays at the honest bucket low (v0.4: `B̂_ref` ×1.156 and the Tier A borrower's remainder 19.49 → 0.22 SOL; v0.4.1: ×0.999); in a genuine pump `P_ema ≤ lo_x` and it never binds.
   **No reserve (v0.4.1; R4A-1, R4B-1).** v0.4 also reserved rent and penalty (`max(V − R − π_ref − ert, ⌊B̂_ref·(10⁴ + ε)/10⁴⌋)`), and v0.3 rent alone. Any reserve creates a band `V − R − π_ref − ert < B̂_live ≤ min(bound, V − K_s − ert)` in which a due liquidation can execute only within ε of a one-second-old reference, while the S1 legs are false; in genuine fast pumps the live cost sits a few tenths of a percent above the reference, so the position gapped into S1 (the reviewers' grid and ECON's 1,655-run battery: 35 extra S1 with the penalty reserve, 1 with rent alone once rent due exceeds `K_s`, 0 without). The cost of no reserve is the held-model truncation of penalty and — on aged positions — rent (ECON §8.2 table, §12.6), disclosed under O19. **Liveness (I11, I-E11):** a liquidatable, buyback-allowed position with a fresh bucket executes as soon as `B̂_live ≤ ⌊B̂_ref·(10⁴ + δ)/10⁴⌋`.
   Require `B̂_live ≤ max_in`, else `ExitBoundNotMet`: the position waits and the keeper retries next second.

Execution:

- **Buy leg** with `max_in`; assert `B ≤ max_in + ert`, at least `q` tokens into `pool_vault` (V2-10).
- Waterfall `liquidate` (§5.2): penalty base `min(B̂_live_risk, B̂_ref)` (quotes, never the measured spend; C0-1 (2), C0-14); the keeper share is `⌊π·keeper_split/10⁴⌋` plus the **keeper floor** top-up `min(max(0, K_s − ⌊π·keeper_split/10⁴⌋), V − B − r − π)` paid from the borrower remainder (A74; R3B-10), credited to `keeper` directly; lender and protocol shares go to `market_vault` (the lender half vests); the borrower remainder stays on `position`, which is closed to `borrower`.
- Market and exposure updates as in `close_short`, with `pool::repay(r, lender_penalty_share)`.
- Event `Liquidate{cr_spot, cr_ema, b_live, b_ref, p_ref, delta, buyback_cost, rent, penalty, split…, keeper_topup, expired}` (v0.4.1: no `pi_ref`).

### 3.7 `expire()`

- Accounts and signer as `liquidate`.
- Preconditions: `now ≥ deadline`; **not** CR-liquidatable; normal buyback allowed; venue not migrating and `venue_executable`; fresh bucket; the expiry reference (ECON §8.2): `P_ref = min(P_ema, min over the ring of p_i, lb_lo)`, `B̂_ref = B̂(Q; reprice(max(L_live, L_ema, L_slow), P_ref))`, `π_ref = ⌈expiry_fee_bps·B̂_ref/10⁴⌉`, `max_in = min(⌊B̂_ref·(10⁴ + delta_exp_bps)/10⁴⌋, V − K_s − ert, max(V − R − π_ref − ert, ⌊B̂_ref·(10⁴ + ε)/10⁴⌋))`; require `B̂_live ≤ max_in`. Rent and the expiry fee stay senior here (v0.4.1 keeps this reserve for `expire` only): nothing is urgent, and an expired position whose price keeps rising becomes CR-liquidatable and exits through `liquidate`, which reserves nothing; no row of the ECON §5.7 battery changes with any expiry reserve rule.
- Buy leg as in `liquidate`.
- Waterfall `expire` (§5.2): expiry fee on `min(B̂_live_risk, B̂_ref)` to lenders 100 % (vests); then the **expiry keeper reward** `K_e` from the borrower's remainder (C0-1 (4)).
- Event `Expire`.

### 3.8 `sol_settle()`

- **Signer:** `keeper` (also pays for a `ShareEpoch` record if this settlement rolls the epoch).
- **Named accounts:** `config`, `market` (mut), `position` (mut; closed to `borrower` only when a settlement executes), `borrower` (mut), `exposure` (mut), `market_vault` (mut), `mint` (read; `supply` for PumpSwap fee tiers), `share_epoch_rec` (Unchecked; key must equal `PDA(["share_epoch", market, market.share_epoch])`), system program, event accounts.
- **Remaining:** `ReadSet` only (mandatory, C0-2). No venue CPI, no token movement.

**One rule: the settlement box (A65; decision 5; ECON §5.6; medians v0.4.1).** A settlement decision may rest on EMA evidence only if manipulated seconds could not have produced it. One attacker-only second moves `P_ema` by up to +16.7 % / −8.3 % (and `L_ema` alike). v0.4 bounded it with the lower medians of the ring's clamped `obs`/`lobs`, but a second that also holds the honest keeper's observation can *freeze* a pushed EMA there (the attacker adds an observation just beyond `P_ema`, the bucket straddles it, `obs = P_ema`), so one push plus 3–4 frozen seconds moved the medians (R4B-2). v0.4.1 takes them over the ring entries' **bucket extremes**: a frozen second holds an honest observation, so its low is at most and its high at least the honest price; moving the lower median of the lows up, or the upper median of the highs down, needs 5 attacker-only seconds of 8 (R4A-9). Every EMA-based decision is evaluated on the box

```
P_mlo = lower median of the ring's lo,  P_mhi = upper median of hi,  L_mlo / L_mhi likewise over llo / lhi
P_lo = min(P_ema, P_mlo), P_hi = max(P_ema, P_mhi), L_lo = min(L_ema, L_mlo), L_hi = max(L_ema, L_mhi)
the box is the point (P_ema, L_ema) if now − ema_ts > RING_LEN seconds: a venue that records no observations
  cannot have been pushed since, and a halted venue's S3 must not be blocked
```

and must come out **the same everywhere in it** — which, because `B̂` and σ increase with `P` and decrease with `L`, means at the corner least favourable to the decision (§4.5 has the predicates). The literal brief wording ("evaluate on the lender-adverse side") was tested in ECON and rejected: for refutation it is the corner that makes refutation *easiest*, and for the amount it lets a borrower's one-second push lower FV by up to 8 % (ECON §5.6).

Procedure (the program's order equals `shortwin_sim.Market.try_exit(settle_only = True)` step for step; it is also the resolution of the crate's open "arming protocol" note):

1. Prologue. Compute `R_due`, `V_eff`, `K_s`, the legs of §4.5 and the box.
2. **Not liquidatable and not expired**: if armed, disarm only if the box says so — the EMA leg fails **and** `CR(V_eff, B̂(Q; reprice(L_lo, P_hi))) ≥ L` (the corner most favourable to liquidation) — emitting `SettleDisarmed{reason: NotLiquidatable}`; otherwise keep the clock (`s_last = now`). Return `Ok`.
3. If the venue is executable (and not `PumpMigrating`):
   - `armed_live = s_armed ∧ now − s_last ≤ settle_arm_gap_s`.
   - If `s_armed` and **refuted** on the box (§4.5): disarm, emit `SettleDisarmed{reason: Refuted}`.
   - Else if the completed bucket is fresh: evaluate the **S1 legs** and **S2 legs** (live, robust spot, and the box corner `(P_lo, L_hi)`). If either holds: arm if not `armed_live` (`s_armed = 1`, `s_count = ema_count`, `s_since = now`); `s_last = now`; if `ema_count − s_count ≥ settle_persist_n`, execute with the kind chosen now — **S1 if the S1 legs hold, else S2**. An S2 whose amount is not box-consistent waits (no further state change). If neither holds and `armed_live`: `s_last = now` (the clock keeps running; nothing refuted it).
   - Else (stale bucket) if `armed_live`: `s_last = now`.

   v0.3 refuted on the raw `P_ema`, so a borrower's one-second dump before maturity disarmed and could be repeated each cycle (at 3 % beyond the S1 boundary no S1 within 600 s); v0.4's box stopped that but not a dump followed by 4 frozen seconds (again no S1 within 600 s); a spot-leg veto longer than the arm gap restarted the clock (+34–42 s). Now S1 lands at most 8 s later under push-then-freeze, and vetoes cost +0–6 s (R3A-6, R4B-2; ECON §12.10).
4. **S3** (market-level, no arming): the position is liquidatable or expired, `s3_no_route` holds **now** (global cap), the continuous chain is unbroken, and `now ≥ max(no_route_since, graduated_at + grad_grace_s, migration_seen_at + max_migration_pause_s·[PumpMigrating]) + s3_wait_s` with the position's locked terms (§5.3). S3 is the only settlement path while `PumpMigrating` (A32). A no-route caused only by the route-broken flag or by the fee hard cap still requires the position to be liquidatable with the spot leg evaluated normally (the flag never makes the spot leg true, V2-1).
5. Otherwise emit `SettleArmed` (if the arming state changed) and return `Ok`. Every "no change" outcome is an `Ok` no-op, not a revert, so the call's prologue — its observation and route chains — is kept and the keeper's per-second call doubles as a crank.

**Amount** (D3; ECON §8.6; checked for S2 and S3): `x(P, L) = min(V − K_s, FV(P, L))` with FV of §4.5, **whose `p'` is decided at that same `(P, L)`** (v0.4.1, R4A-3: the raw "CR-liquidatable" flag's robust-spot leg fails on one contrary low observation, which in v0.4 switched both corners to the expiry fee and cut the lenders' S3 take by 2.9–7.3 %). Settle only if `(x(P_hi, L_lo) − x(P_lo, L_hi))·10⁴ ≤ fv_dev_bps·x(P_lo, L_hi)` (exact integers); pay `x(P_ema, L_ema)` (D3 literal: EMA reserves). S1 pays `V − K_s`, never capped, and needs its EMA leg at `(P_lo, L_hi)`, so a lender-keeper's pushed or frozen seconds cannot flip a capped S2 into an uncapped S1 (v0.3: +2.37 to +5.58 SOL on the drained RA cases after 1–2 pushes; v0.4: the same after 1 push + 4 frozen seconds; v0.4.1: only after 5 consecutive attacker-only seconds, a degraded-mode exposure) (R3A-1, R4B-2, R4A-9; ECON §12.11). A one-second push still moves an S2/S3 amount by at most `fv_dev_bps` (±1 %); the realistic Emergency-flag S3 case moves +0.70 SOL instead of +10.48 SOL without the box. **Residual (disclosed):** a contrary observation in every second widens one side of the box and defers S1 or an S3 on an executable venue for as long as it is paid (0.002–0.21 SOL/s at a 1 % round trip; ECON §5.6).

Effects of an executing settlement (waterfall `settle`, §5.2):

- `K_s` (locked terms) to the keeper.
- Lenders: **S1** `x = V − K_s`; **S2/S3** `x = x(P_ema, L_ema)`. It goes to `market_vault` through `pool::settle(x, lump)`: S1 `lump = 0`; S2/S3 `lump = x − min(x, ⌊Q·P_ema/WAD⌋ + R_due)` — impact, fees and penalty above the zero-impact value of the extinguished tokens — which vests; the rest is immediate (§3.3; v0.4.1, R4B-6: a token whale's `[deposit → sol_settle → withdraw → claim]` took 90 % of the premium over market value; this formula also needs no `B̂_FV` reconstruction, R4A-5).
- `V − K_s − x` to the borrower through the position close. The protocol gets nothing (SPEC §6.3).
- `pool::maybe_roll()`: if it rolls, create `share_epoch_rec` with the grief-proof helper (payer keeper, ≈ 0.0013 SOL) and write `(acc_end, acc_r_end, s_old, num, den)` (A39, C0-6); a full wipe flushes the vest into the record first and sets `wipe_live` (V2-12, R3B-2).
- `rent_aggr −= q·coef·(10⁴ − p)`, `open_positions −= 1`, exposure update.
- Event `SolSettle{condition, b_live, b_rs, b_ema, p_lo, p_hi, l_lo, l_hi, x_lo, x_hi, pprime_lo, pprime_hi, fv, k_s, lender_amount, lump, borrower_amount, ema_age_s}` — `ema_age_s = now − ema_ts` reports how stale the EMA is when the venue was not observable (V2-5).

"Attempt the normal buyback first" (SPEC §5.3) is met by the on-chain proof: the S1 and S2 legs are exactly the negation of "normal buyback allowed" on the live leg, confirmed on the robust spot and on the box; a failed CPI cannot be recorded because it reverts the transaction [R-JUP §5.1; ECON §5.6]. S3 on an executable venue (flag or fee above the hard cap) does not require it: a keeper may race an ordinary expiry with `sol_settle(S3)`; at the expiry-fee reading of FV (O15) lenders then receive in SOL what the expiry would have paid (ECON §8.6).

### 3.9 Cranks and helpers

| Instruction | Signer | Accounts | Effect |
|---|---|---|---|
| `crank_price()` | anyone | `config`, `market` (mut), `mint`, `oracle_cache` (read), event accounts. Remaining: `ReadSet(venue)`; for `None` markets the detection set `[bonding_curve, canonical_pool]` | Prologue (accrue incl. vest release, roll, sync venue incl. `→ PumpSwap` and `None →` pump re-detection, record only if the venue is executable, route chains), tier refresh. A correctly keyed venue account that no longer parses is recorded as "not executable", never a revert (A63, V1-8), so a batch of cranks (§7.1) fails only on wrong keys. Event `PriceCranked`, plus `VenueStateChanged` / `RouteChainChanged` on changes |
| `crank_sol_usd()` | anyone | `config`, `oracle_cache` (mut), Pyth `PriceUpdateV2` | Hand-parsed (A29): owner == receiver constant, address == `pyth_sol_usd`, discriminator `PriceUpdateV2`, `verification_level == Full`, feed id match, `publish_time ≥ now − oracle_max_age_s` (120 s), `conf/price ≤ oracle_max_conf_bps`, `abs(price/cached − 1) ≤ oracle_max_step_bps` (skipped if the cache is older than `oracle_stale_s`), `publish_time > cached.publish_time`. Keepers retry: the sponsored feed updates on a 55 s heartbeat or a 0.5 % move (C1-12) [R-VEN Review corrections 2] |
| `flag_unsafe_mint()` | anyone | `market` (mut), `mint` | If `check_mint_safety` **fails**, set `mint_unsafe` (blocks opens and deposits) and emit `MintSafetyTripped`; otherwise `MintSafe` error (ADOPTED Q7) [R-MINT §6] |
| `flag_venue_upgraded(venue)` ★ | anyone | `venue_guard` (mut), that program's ProgramData | The venue-upgrade latch (§3.1, A7) |
| `propose_pool()` | anyone | `config`, `market` (mut), `mint`, event accounts. Remaining: candidate ReadSet (+ LP mint), and the incumbent's ReadSet if the market is Raydium | Pool selection (§2.6): adopt (listing, listing window with its 5 % margin, a `None` market, or — **only while `open_positions == 0`** — a candidate with 25 % more locked depth), or open / improve a fast-path collection window (incumbent `pool_fail` for ≥ 120 s). Records an observation on the incumbent if it is executable. Rejected on pump-venue markets (`PumpVenuePrecedence`); with open positions and no failure, `PositionsOpen` |
| `execute_pool_switch()` | anyone | as `propose_pool`, with the fast-path candidate | **Fast path only**: after the 60 s window, adopts the best candidate seen if it is still eligible and the incumbent still fails `pool_fail`; otherwise clears the window. Events `PoolSwitched` / `PoolSwitchCancelled` |
| `claim_venue_rewards()` | anyone | `market` (mut), `market_vault` (mut), `trader` (mut), the trader's pump or pAMM UVA (+ its WSOL ATA), `trader_wsol` (PumpSwap), venue program and event authority, System, Tokenkeg, WSOL mint; the latest `ShareEpoch` record while `S == 0` | CPI pump `claim_cashback` (lamports to the trader) or pAMM `claim_cashback` (WSOL to the ephemeral `trader_wsol`, then closed), then normalise: everything is credited to lenders through `vest` (§3.3; A44, C0-18 (1), R3B-2); while a full-wipe record is live it credits the record and sets `wipe_rewards_claimed` even if nothing was claimable (v0.4.1, R4A-7). The pinned IDLs do not require the user's signature [V11]; the trader signs anyway. **UNVERIFIED** on a fork (T-CASHBACK) |
| `sweep_trader()` | anyone | `market` (mut), `market_vault` (mut), `trader` (mut), `ATA(trader, WSOL)` (mut), `trader_base_ata` (pump venues), `pool_vault`, Tokenkeg, System; the latest `ShareEpoch` record while `S == 0` | If `ATA(trader, WSOL)` exists, `CloseAccount → trader` (trader signs); then the §2.4 normalisation (SOL through `vest`); while a full-wipe record is live it credits the record and sets `wipe_trader_swept` (v0.4.1, R4A-7). Lets anyone recover donated or claimed WSOL; trading legs never depend on it (A33) |
| `set_coverage_pause(paused, reason)` ★ | `coverage_authority` | `config`, `market` (mut) | Degraded mode (§3.1, A69; inert until O17) |

The straddle's curve completion is **not** a short.win instruction: it is the separate `complete-curve` helper (§5.4, A66), so the core program never forwards a user's signature to a venue.

### 3.10 Events (via `emit_cpi!`, one per instruction; `CoverageGap` via `emit!`)

The common fields are `market`, `slot`, `ts`. v0.4 additions and changes are marked ★, v0.4.1 ★★.

| Event | Fields |
|---|---|
| `MarketCreated` | mint, token_program, venue, creator, bonding_curve, canonical_pool, ray_pool, tier, creation_fee |
| `Deposit` / `Withdraw` / `ClaimSol` / `DeficitCovered` / `LenderSynced` | owner, amount, shares, T_after, S_after, sol_paid, ★pot_draw, lock, ★lock_until, epoch |
| `Open` | position, borrower, nonce, q, s, m, v, n_open, fee_open, tier, entry_price, spot_wad, ema_wad, b_spot, b_ema, deadline, venue, venue_rent (PumpSwap `r`, V1-13) |
| `Close` | position, borrower, q, buyback_cost, rent, rent_protocol, borrower_payout |
| `Liquidate` | position, keeper, cr_spot_bps, cr_ema_bps, b_live, b_ref, ★p_ref, delta_bps (fixed), buyback_cost, rent, penalty, penalty_{lender,protocol,keeper}, ★keeper_topup, borrower_payout, expired, token_surplus |
| `Expire` | position, keeper, b_live, b_ref, ★★pi_ref, buyback_cost, rent, expiry_fee, keeper_reward, borrower_payout |
| `SolSettle` | position, keeper, condition (1/2/3), b_live, b_rs, b_ema, ★p_lo, ★p_hi, ★l_lo, ★l_hi, ★x_lo, ★x_hi, ★★pprime_lo, ★★pprime_hi, fv, k_s, lender_amount, ★lump, borrower_amount, ema_age_s |
| `SettleArmed` / `SettleDisarmed` | position, s_count, s_since, reason (★Refuted / ★NotLiquidatable) |
| `EpochRolled` | epoch, acc_end, num, den, ★vest_flushed |
| `ProtocolFeesSwept` / `MarginAdded` | amount, recipient / position, amount |
| `PriceCranked` | spot_wad, ema_price, ema_liq, liq_slow, ema_count, lb_lo, lb_hi, tier, util_bps, rent_basis[3], ★vest_bal |
| `VenueStateChanged` / ★`RouteChainChanged` | from, to, pool, graduated_at / no_route_since, last_no_route_obs, pool_fail_since, last_pool_fail_obs |
| `PoolProposed` / `PoolSwitched` / `PoolSwitchCancelled` | candidate, kind (listing / zero-position / fast), d_cand, d_inc, window_ends |
| `TraderSwept` / `VenueRewardsClaimed` | lamports, tokens / lamports, ★to (vest / wipe record / protocol), ★★wipe_bits |
| `MintSafetyTripped` | mint |
| ★`VenueUpgradeFlagged` / `VenueRevalidated` | venue, guard_slot, programdata_slot, flagged_at, ★★expected_slot (revalidate) |
| ★`CoverageGap` (log, `emit!`) | gap_s, open_positions |
| ★`CoveragePauseSet` | paused, reason |
| `ConfigProposed` / `ConfigApplied` / `ConfigCancelled` / `PauseSet` / `VenueDisabled` / `RouteBrokenSet` / `VenueExtrasSet` | change kind + hash, not_before / scope, flag / venue / keys |

### 3.11 Error codes (one `#[error_code]` enum; Anchor 1.x allows one per program [R-TOOL §1.3])

| Group | Codes |
|---|---|
| Math/accounts | `MathOverflow`, `AccountMismatch`, `VenueAccountMismatch`, `DeniedAccount`, `BadRemainingAccounts`, `BadExtras`, `ConfigVersion`, `EpochRecordMismatch` |
| Admin | `Unauthorized`, `TimelockNotElapsed`, `NoPendingChange`, `InvalidParams`, `TimelockTooShort`, ★`VenueNotUpgraded`, ★★`VenueSlotMismatch`, ★`CoverageAuthorityDisabled` |
| Mint | `MintUnsupportedProgram`, `MintNative`, `MintFreezeAuthority`, `MintDisallowedExtension`, `MintMalformed`, `MintSafe`, `MintUnsafeFlagged` |
| Venue | `NoSupportedVenue`, `PumpVenuePrecedence`, `CurveMayhemUnsupported`, `CurveQuoteNotSol`, `CurveNeedsExtend`, `VenueMigrating`, `NoRoute`, `RouteBroken`, `CurveStraddle`, `VenueDisabled`, `VenueUpgraded`, `VenueFlagged`, `VenueFeeTooHigh`, `BoostInProgress`, `VenueResultMismatch`, `VenueRentOnSell`, `TraderStateInvalid`, `DelegateNotCleared`, `AuthorityTampered` |
| Pool selection | `PoolIneligible`, `PoolNotDominant`, ★`PositionsOpen`, `PoolSwitchNotDue`, `NoPendingSwitch`, `PoolListWindow`, `PoolLockTooLow`, `PoolTooThin` |
| Opens | `OpensPaused`, ★`CoveragePaused`, `MarketWarmingUp`, `PriceDeviatesFromEma`, ★`PriceAboveOpenMax`, `CapPositionImpact`, `CapMarketOi`, `CapUtilization`, `CapCurveGamma`, `CapWallet`, `InsufficientInventory`, `BelowMinNotional`, `MarketNotionalCap`, `ProceedsBelowMin`, `MarginAboveMax`, `OpenFeeAboveMax` |
| Exits | `InsufficientVault`, `BuybackAboveMax`, `NotLiquidatable`, `UseSolSettle`, `UseExpire`, `NotExpired`, `ReferenceStale`, `ExitBoundNotMet`, `SettlementNotAllowed`, `S3TimerRunning` |
| Lenders | `ZeroAmount`, `SharesBelowMin`, `TokensBelowMin`, `InsufficientShares`, `InsufficientFreeInventory`, `AboveProRataAllowance`, `NegativePending`, `LenderBehindEpochs`, ★★`WipeSweepPending` |
| Oracle | `OracleOwner`, `OracleFeed`, `OracleStale`, `OracleConfidence`, `OracleStep`, `OracleVerification` |

### 3.12 Venue adapters

| Adapter | MVP? | Sell (open) | Buy (close / liq / expire) | Signer | Pricing source | Venue-side rent |
|---|---|---|---|---|---|---|
| `PumpCurve` | **yes** | `sell_v2(q, min_sol_output)`: lamports to `trader` | `buy_v2(q, max_sol_cost)`: System transfers from `trader` | `trader` | Live curve, exact formula [R-PUMP §4] | ≤ `ert` |
| `PumpSwap` (canonical pool) | **yes** (D1) | `sell(q, min_quote_out)` into `trader_wsol` | `buy(q, max_quote_in, [false])`, exact base out | `trader` | Same pool, effective reserves [R-VEN §2.4] | ≤ `ert` |
| `RaydiumV4` | **yes** (D2) | tag 16 `swap_base_in_v2` | tag 17 `swap_base_out_v2` | `trader` | Adopted pool, `vault − need_take_pnl` [R-RAY §2.2, §3.3] | 0 |
| `RaydiumCpmm` | **yes** (D2) | `swap_base_input` | `swap_base_output` | `trader` | Adopted pool, `vault − fee counters` [R-RAY §2.4, §3.3] | 0 |
| `Jupiter` | v1.1 (D1) | `route_v2` / `shared_accounts_route_v2` | ExactIn; surplus `out − Q` to the borrower (ADOPTED Q20) | per-position `swap_auth` | Bounded by the on-chain quote and the §3.6 bound | — |
| Raydium CLMM, Orca Whirlpools, Meteora DLMM / DAMM v2 | list-only (D2) | — | — | — | — | — |

**PumpCurve account sets** (order is binding; S = signer, W = writable; every slot mandatory). Since v0.4 no venue has an `OpenTail`: ProgramData accounts are read only by `flag_venue_upgraded` and `revalidate_venue` (A7).

- **ReadSet**: C0 `bonding_curve` (W in trades); C1 pump `global` (`creator_fee_configurable` @1045, `pool_migration_fee` @146, fee and buyback recipient lists [R-PUMP §2.2]); C2 pump `fee_config` (pfee); C3 canonical pool (derived; may be uninitialised; used for detection); C4 `curve_base_ata` (W in trades; its amount feeds `B̂_str`'s `X_p`); **C5 pAMM `fee_config` and C6 pAMM `global_config` (mandatory: the straddle quote `B̂_str` is always defined, C0-2)**.
- **TradeExtras**: T0 pump program; T1 pump `event_authority`; T2 pfee program; T3 WSOL mint; T4 Tokenkeg; T5 ATA program; T6 `fee_recipient` (W; must be in the live `Global` list, checked by short.win as well as pump, C0-14); T7 its WSOL ATA; T8 `buyback_fee_recipient` (W; live list); T9 its WSOL ATA; T10 `curve_quote_ata`; T11 `ATA(trader, WSOL)` (seed-checked only by pump; SOL coins ignore it and it need not exist [R-PUMP §6.3]); T12 `creator_vault` (W; **re-derived from `curve.creator` at execution**); T13 its WSOL ATA; T14 `sharing_config`; T15 pump GVA (buy only); T16 trader pump UVA (W; must exist, pre-created at listing); T17 UVA WSOL ATA; T18 `trader_base_ata` (W); T19 System.
- **CPI**: built exactly as the `buy_v2` (27 accounts) / `sell_v2` (26) tables of the IDL [R-PUMP §3.5; V11], with `mint`, `token_program`, `trader` and `trader_base_ata` inserted. Discriminators `buy_v2` `b817ee6167c5d33d`, `sell_v2` `5df6823ce7e940b2`.
- **Legacy fallback (A31)**: legacy `buy`/`sell` (16/14 accounts + 2 remaining: `bonding_curve_v2`, which pre-dates 2026-04-28, then `buyback_fee_recipient`, required since 2026-04-28 (C1-10) [R-PUMP Review corrections 4]) behind `pump_ix_version`, only if pump deprecates v2.

**PumpSwap account sets.**

- **ReadSet**: P0 `pool` (W in trades); P1 `pool_base_vault` (W); P2 `pool_quote_vault` (W); P3 pAMM `global_config` (`disable_flags` @56, fee and buyback recipient lists, `creator_fee_configurable` @940); P4 pAMM `fee_config` (pfee); **P5 `boost_vault_ata` (derived; mandatory; may be missing)**.
- **TradeExtras**: T0 pAMM program; T1 pAMM `event_authority`; T2 pfee program; T3 WSOL mint; T4 Tokenkeg; T5 ATA program; T6 System; T7 `trader_base_ata` (W); T8 `trader_wsol` (W); T9 `protocol_fee_recipient` (from the live `GlobalConfig` list, the reserved list for mayhem pools which are rejected anyway); T10 its WSOL ATA (W); T11 `coin_creator_vault_ata` (W); T12 `coin_creator_vault_authority` = pAMM `["creator_vault", pool.coin_creator]`; T13 pAMM GVA (buy); T14 trader pAMM UVA (W; buy); T15 `pool-v2` marker (in the CPI only if `coin_creator ≠ default`); T16 `buyback_fee_recipient` (live list); T17 its WSOL ATA (W); T18 cashback UVA WSOL ATA (W; in the CPI only if `pool.is_cashback_coin`).
- **Existence requirement (C0-14, C1-8, C1-9):** T10, T11, T14, T17 and (cashback coins) T18 must already be initialised; otherwise the instruction fails with `VenueAccountMismatch` and the client or keeper prepends `create_idempotent` / `init_user_volume_accumulator` at its own cost. PumpSwap then never creates accounts at the trader's expense and the stack stays at height 3 (§6.3). All eight protocol-fee WSOL ATAs already exist [R-VEN Review corrections 5].
- **CPI order**: exactly [R-VEN §2.2]: 23 fixed accounts for `buy`, 21 for `sell`, then remaining accounts cashback → pool-v2 → buyback recipient → its ATA. Discriminators `buy` `66063d1201daebea`, `sell` `33e685a4017f83ad`. Pools under 300 bytes need `extend_account` prepended by the client [R-VEN §1.2].

**RaydiumV4 account sets** [R-RAY §3.1, §9.2].

- **ReadSet**: R0 `pool` (`AmmInfo`; W in trades); R1 `coin_vault` (W); R2 `pc_vault` (W); R3 `lp_mint`.
- **TradeExtras**: X0 AMM v4 program `675kPX9…`; X1 authority `5Q544f…`; X2 Tokenkeg; X3 WSOL mint; X4 System; X5 `trader_wsol` (W).
- **CPI** (8 accounts, v2 tags only): `[Tokenkeg, pool W, authority, coin_vault W, pc_vault W, source W, destination W, trader S]`; data `[16 | amount_in u64 | minimum_amount_out u64]` (sell) or `[17 | max_amount_in u64 | amount_out u64]` (buy). Sell: `source = pool_vault` (trader as delegate), `destination = trader_wsol`. Buy: `source = trader_wsol`, `destination = pool_vault`. The legacy tags 9/11 are never used: they take exactly 17/18 accounts, 8 of them ignored, and accept no extras [R-RAY §3.1, §9.6].

**RaydiumCpmm account sets** [R-RAY §3.2, §9.2].

- **ReadSet**: R0 `pool` (`PoolState`; W in trades); R1 `amm_config`; R2 `token_0_vault` (W); R3 `token_1_vault` (W); R4 `lp_mint`.
- **TradeExtras**: X0 CPMM program `CPMMoo8…`; X1 authority `GpMZb…`; X2 `observation` (W); X3 Tokenkeg; X4 the token side's program (== `pool.token_x_program` == `market.token_program`); X5 WSOL mint; X6 System; X7 `trader_wsol` (W). The token mint is the named `mint`.
- **CPI** (13 accounts): `[trader S (payer), authority, amm_config, pool W, input_token_account W, output_token_account W, input_vault W, output_vault W, input_token_program, output_token_program, input_mint, output_mint, observation W]`; discriminators `swap_base_input` `8fbe5adac41e33de` (args `amount_in, minimum_amount_out`), `swap_base_output` `37d96256a34ab4ad` (args `max_amount_in, amount_out`). Anchor accepts trailing extras [R-RAY §9.6].

**Seed gotchas.** Pump's creator vault seed is `"creator-vault"` (hyphen) [R-PUMP §3.1]; PumpSwap's is `"creator_vault"` (underscore) [R-VEN §2.2]; the CPMM `amm_config` PDA uses the index as u16 **big-endian** [R-RAY §2.5]. AMM v4 pool addresses are not derivable from the mint, and CPMM pools may sit at random keypair addresses [R-RAY §2.1, §2.4]: Raydium pools are always identified by the stored keys of §2.6.

**Jupiter adapter (v1.1 design; [R-JUP §4.3, §7.2]).** Per-position `swap_auth = ["swap_auth", position]` with temporary in/out token accounts created and closed in-instruction; program pinned to `JUP6…`; only `route_v2` `[187,100,250,204,49,196,175,20]` and `shared_accounts_route_v2` `[209,152,83,147,124,254,216,233]`; `in_amount == funded`, `platform_fee_bps == 0`, `positive_slippage_bps == 0`; signer flag only for `swap_auth`; denylist. Bounds come from the on-chain quote of the market's venue, never from the route: opens `min_out = quote(Q)·(1 − ε)`; exits `max_in` of §3.6 / §3.7 and `out ≥ Q`, surplus `out − Q` to the borrower (ADOPTED Q20). Top-level only: short.win 1 → Jupiter 2 → AMM 3 → Token 4 [R-JUP §3.2].

---

## 4. On-chain pricing and tiers

Notation follows ECON §2: `X` = token reserve, `Y` = SOL reserve (pricing reserves), `P = Y/X` (WAD), `L = √(X·Y)`.

### 4.1 Reading venue state (`shortwin-venue-state`)

All readers take `&[u8]`, check the owner, the length and (where one exists) the 8-byte discriminator, and **zero-pad** reads past the end of shorter accounts [R-TOOL §1.4; R-VEN §1.2]. A reader failure on a correctly keyed venue-state account returns `NotExecutable` to the route logic instead of an error (A63, V1-8); only key mismatches revert.

| Account | Owner / discriminator | Fields read (byte offset) | Source |
|---|---|---|---|
| pump `BondingCurve` | pump / `17b7f83760d8ac60` | vT @8, vQ @16, rT @24, rQ @32, supply @40, complete @48, creator @49, is_mayhem @81, is_cashback @82, quote_mint @83, creator_fee_bps @115 | [R-PUMP §2.1] |
| pump `Global` | pump / `a7e8e8b1c86c727f` | pool_migration_fee @146, recipient lists, creator_fee_configurable @1045 | [R-PUMP §2.2] |
| pfee `FeeConfig` | pfee / `8f3492bbdb7b4c9b` | flat_fees @41, `Vec<FeeTier>` fee_tiers (u32 len; u128 threshold + 3×u64 each), stable tiers, exotic flat fees | [R-PUMP §2.3] |
| PumpSwap `Pool` | pAMM / `f19a6d0411b16dbc` | index @9, creator @11, base_mint @43, quote_mint @75, lp_mint @107, base vault @139, quote vault @171, coin_creator @211, is_mayhem @243, is_cashback @244, virtual_quote_reserves i128 @245, creator_fee_bps @261 | [R-VEN §1.2] |
| PumpSwap `GlobalConfig` (949 B) | pAMM / `95089ccaa0fcb0d9` | disable_flags u8 @56, protocol_fee_recipients @57, buyback_fee_recipients @643, creator_fee_configurable @940 | [R-VEN §2.2; C1 verified] |
| Raydium AMM v4 `AmmInfo` ★ | `675kPX9…`, **exactly 752 B, no discriminator** | status u64 @0, nonce @8, swap_fee_numerator @176, swap_fee_denominator @184, need_take_pnl_coin @192, need_take_pnl_pc @200, pool_open_time @224, coin_vault @336, pc_vault @368, coin_vault_mint @400, pc_vault_mint @432, lp_mint @464, lp_amount @720 | [R-RAY §2.1] |
| Raydium CPMM `PoolState` ★ | `CPMMoo8…` / `f7ede3f5d7c3de46`, 637 B | amm_config @8, token_0_vault @72, token_1_vault @104, lp_mint @136, token_0_mint @168, token_1_mint @200, token_0_program @232, token_1_program @264, observation_key @296, status u8 @329, lp_supply @333, protocol_fees_token_0/1 @341/@349, fund_fees_token_0/1 @357/@365, open_time @373, creator_fee_on u8 @389, enable_creator_fee @390, creator_fees_token_0/1 @397/@405 | [R-RAY §2.4] |
| Raydium CPMM `AmmConfig` ★ | `CPMMoo8…` / `daf42168cbcb2b6f`, 236 B | trade_fee_rate @12, protocol_fee_rate @20, fund_fee_rate @28, creator_fee_rate @108 (ppm) | [R-RAY §2.5] |
| SPL/T22 token account | Token/T22 | mint @0, owner @32, amount @64, delegate COption @72, delegated_amount @121, close_authority COption @129 | SPL layout |
| SPL/T22 mint | Token/T22 | supply @36, decimals @44, freeze_authority @46 | [R-MINT §3.1] |
| `ProgramData` | BPFLoaderUpgradeable | tag u32 @0 (== 3), slot u64 @4 | [R-PUMP §1; V9] |
| Pyth `PriceUpdateV2` | `rec5EKMG…` | verification_level @40, feed_id, price i64, conf u64, expo i32, publish_time i64 | [R-VEN §5] |

**Pricing reserves and real SOL** (`VenueState { X, Y, R, P, L, fee }`):

| Venue | Pricing `X` (token), `Y` (SOL) | Real SOL `R` (tiers, pool eligibility) | Undefined / no route when |
|---|---|---|---|
| PumpCurve | `(vT, vQ)` | — (Tier D) | `complete` |
| PumpSwap | `(base vault, quote vault + VQR)`; VQR read live on every quote, never cached (it can drift upward after the boost, C1-13) [R-VEN §1.4] | `quote vault.amount`; sell proceeds are capped by it | pool invalid, base vault 0 |
| RaydiumV4 | `vault − need_take_pnl` per side, SOL side by `coin/pc_vault_mint` | `Y` | `vault < need_take_pnl` (the venue would revert with `CheckedSubOverflow`) |
| RaydiumCpmm | `vault_i − (protocol_i + fund_i + creator_i)` per side | `Y` (e.g. pool `FQd8z3…`: vault 792.6 SOL, `R` 547.1 SOL [R-RAY §2.4]) | `vault_i < counters_i` (the venue reverts with `InsufficientVault`) |

Raydium has no virtual reserves: real equals effective [R-RAY §3.3].

### 4.2 Fees (read on every quote, never cached)

- **Pump curve** [R-PUMP §4]: `mcap = vQ·1e15/vT`; `fee_tiers` for SOL quote; tier 0 below `tiers[0].threshold`, else the **last** tier with `threshold ≤ mcap`; `creator_bps` replaced by `curve.creator_fee_bps` if `Global.creator_fee_configurable ∧ curve.creator_fee_bps ≠ 0`; `creator_bps = 0` if `curve.creator == default`. Today 95 + 30 bps.
- **PumpSwap** [R-VEN §2.3]: `mcap = effective_quote·base_mint.supply/base_reserve` (live supply); same tier rule; per-pool creator override if `creator_fee_configurable ∧ pool.creator_fee_bps > 0`; `creator_bps = 0` if `coin_creator == default`. Today 2/93/30 bps below a 420 SOL market cap, down to 20/5/5 at ≥ 98,240 SOL. Fees are reimplemented rather than obtained by CPI to `get_fees_with_quote_mint`; a mismatch is caught by differential tests and by the venue guard on pfee upgrades.
- **Raydium AMM v4** [R-RAY §2.3]: the trader pays `swap_fee_numerator/denominator` of the input, read per pool (25/10,000 on every sampled pool). The whole fee stays in the vault; the protocol later removes 12 % of the constant-product growth through `withdrawpnl`, which only moves `need_take_pnl`. No creator or fund fee, no fee accounts. One-way fee `φ = ⌈num·10⁶/den⌉` ppm (bps for display only; ECON §2.1).
- **Raydium CPMM** [R-RAY §2.5, §3.3]: `t = amm_config.trade_fee_rate`; `c = amm_config.creator_fee_rate` if `pool.enable_creator_fee`, else 0 (ppm). The creator fee is on the input when `creator_fee_on == 0`, or when it names the input token; otherwise on the output. Protocol (12 %) and fund (4 %) shares are carved out of `t` into the excluded counters and do not change the trader's cost. All 21 live configs are listed in [R-RAY §2.5] (trade fee 0.005 %–4 %, creator fee 0.05 %–1.495 % when enabled). One-way fee `φ = t + c` ppm exactly, with no rounding (R3A-3 iii). Configs above `venue_fee_cap_bps` are simply ineligible.
- The buyback split inside pump's protocol fee does not change the trader's cost [R-PUMP §4; R-VEN §2.3].
- **Units.** Every venue's one-way fee `φ` is carried in ppm (pump: bps × 100 exactly), and every gate compares it with `cap_bps·100`, so no gate rounds (ECON §2.1). Quotes on hypothetical states — ring entries, box corners, exit references, FV — use the **live** fee tier (R3B-7 b).

### 4.3 Executable cost and sale proceeds

All formulas are exact and bit-for-bit reproduced against mainnet trades: pump curve and PumpSwap [R-PUMP §4.1; R-VEN §2.4]; Raydium AMM v4 on 388 swaps and CPMM on 285 swaps plus SIMs [R-RAY §3.4]. `fee(x, b) = ⌈x·b/10⁴⌉`; `D = 10⁶`; `pre(a, r) = ⌈a·D/(D − r)⌉`, `pre(a, 0) = a`.

```
Buy Q tokens, exact out (defined only for 0 < Q < X; curve also Q ≤ rT):
  Curve        n = ⌊Q·vQ/(vT − Q)⌋ + 1            ; B̂ = n + fee(n, prot) + fee(n, creator)
  PumpSwap     n = ⌈Y·Q/(X − Q)⌉ (Y = real + VQR) ; B̂ = n + fee(n, lp) + fee(n, prot) + fee(n, creator)
  Raydium v4   pre = ⌈Y·Q/(X − Q)⌉                ; B̂ = ⌈pre·den/(den − num)⌉
  CPMM, creator fee on input (or off)
               sw  = ⌈Y·Q/(X − Q)⌉                ; B̂ = pre(sw, t + c)
  CPMM, creator fee on the token (output) side
               Q'  = pre(Q, c), require Q' < X ; sw = ⌈Y·Q'/(X − Q')⌉ ; B̂ = pre(sw, t)      (source only: UNVERIFIED on mainnet)

Sell q tokens, exact in:
  Curve        g = ⌊q·vQ/(vT + q)⌋ ; S = g − fee(g, prot) − fee(g, creator)
  PumpSwap     g = ⌊Y·q/(X + q)⌋   ; S = g − Σ fees ; require Y_real ≥ g − fee(g, lp)   (real-reserve cap)
  Raydium v4   net = q − ⌈q·num/den⌉ ; S = ⌊Y·net/(X + net)⌋
  CPMM, fee on input  (creator fee on the token side, or both)
               net = q − ⌈q·(t + c)/D⌉ ; S = ⌊Y·net/(X + net)⌋
  CPMM, fee on output (creator fee on the SOL side)
               net = q − ⌈q·t/D⌉ ; sw = ⌊Y·net/(X + net)⌋ ; S = sw − ⌈sw·c/D⌉

Straddle quote B̂_str (curve live, Q > rT) — the executed path of §5.4 (A14 as amended, V1-9):
  B̂_str(Q) = B̂_pool(Q;  X_p = migrated tokens, Y_p = rQ + n_curve(rT) − pool_migration_fee,
                          pAMM fee tier at mcap(Y_p, X_p))
  i.e. the keeper completes the curve with its own SOL (tx1), migrate_v2 (tx2), then the position buys ALL of Q
  on the new canonical pool at the completion price (tx3). v0.2's B̂₂ = B̂_curve(rT) + B̂_pool(Q − rT) priced a path
  the MVP does not have (the position buying rT on the curve) and understated the cost; it is kept only for the
  v1.1 in-program straddle.
  live leg: rT, rQ from the curve, X_p = curve_base_ata.amount − rT                    (C4, C5, C6 mandatory)
  EMA leg : (vT, vQ)_ema = curve_reprice(L_curve, P_ema); rT_ema = vT_ema − (vT − rT); rQ_ema = vQ_ema − (vQ − rQ);
            X_p = 206,900,000,000,000 (standard curve), Y_p from rQ_ema and n_curve(rT_ema)  — deterministic (C0-2)
```

- ECON §3.1's generic Raydium form (`n = ⌈Q·Y/(X − Q)⌉`, `B̂ = ⌈n·D/(D − f)⌉`) is the fee-on-input case above; AMM v4 uses its own `num/den` instead of ppm, and the CPMM creator-fee-on-output case grosses `Q` up first. ECON v0.2.1 §3.1 points here.
- The on-chain quote equals what the venue charges in the same instruction, because both read the same pre-trade state. Exits pass `max_in` (≥ the quote) as the venue's slippage bound; opens pass `min_out = quote·(1 − ε)`. `ε = exec_tol_bps` only absorbs rounding drift after a venue upgrade; on Raydium `ε = 0` would be exact (673 reproduced trades), and the configured 10 bps is kept as a common guard [R-RAY §9.3].
- `B̂ = +∞` (no quote) only if `Q ≥ X` (or `Q' ≥ X`), the curve is complete, or the venue is **not executable** (§5.3). The route-broken flag and the fee hard cap never make a quote infinite (V2-1). A missing account is never "+∞" (C0-2). One exception reads an undefined quote differently: the D4 margin's EMA leg, whose undefined quote leaves the margin on `B̂_spot` alone (§3.4 step 10, as the sim's `Market.open`); every CR, leg and box predicate keeps `+∞`.
- **Two fee views (V2-1).** *Execution quotes* (`B̂_live` in close affordability, "normal buyback allowed" and the `B̂_live ≤ max_in` checks) use the live venue fee, because that is what the venue charges. *Risk quotes* — every CR (spot and EMA legs), the S1 and S2 legs, the settlement-box corners, the exit reference `B̂_ref`, the refutation and FV — use `φ_risk = min(φ_live, venue_fee_hard_cap_bps)` (the position's locked cap). An admin fee hike above the cap therefore cannot make positions liquidatable or trip S1; it makes buybacks exceed their bound, so positions wait and the S3 chain (which counts the hike as no-route) runs.
- σ (SPEC S2 "slippage") is ECON's fee-free average premium: `σ = ⌊n·WAD·10⁴/(Q·P)⌋ − 10⁴` bps with `P = ⌊Y·WAD/X⌋` of the same state (ECON §2.1), with `n` the pre-fee input (`pre` / `sw` on Raydium; for `B̂_str` the pool's `n·WAD/(q·P) − 1` with `P` the current curve price).
- **Straddle dip at `Q = rT` (recorded; the envelope is the owner's choice, F-A2).** Above `rT` all of `Q` is priced on the post-migration pool, whose `Y_p = rQ + n(rT) − fee` starts ≈ 0.023 % below the curve's completion price, so `B̂_str(rT + 1)` can be below `B̂_curve(rT)` (by at most ≈ 3 bps, proptested in `shortwin-math`), and on the EMA leg `B̂` can fall as `P_ema` rises past the price where `rT(P) = Q`. The liquidation price shown to users is therefore searched on the curve and the pool regimes separately. A monotone envelope `max(B̂_pool(q), B̂_curve(rT))` for `q > rT` is the alternative (not built).
- Round trips: an immediate sell-then-buy of `Q` costs only the fee pair (curve: 2.53 % of proceeds; 5.05 % is the same-state bid/ask spread for size Q, C1-6) [ECON §3.2; R-PUMP §4.2].

### 4.4 Observation buckets, EMA and ring (ECON §5)

```
record(P, L):                          // prologue step 7, pre-trade state, every instruction with a ReadSet,
                                       //   only while the venue is executable (or PumpMigrating) — A64, V2-5
    if b_sec != now: b_sec = now; b_lo = b_hi = P; b_llo = b_lhi = L
    else: b_lo = min(b_lo, P); b_hi = max(b_hi, P); b_llo = min(b_llo, L); b_lhi = max(b_lhi, L)

integrate(now):                        // prologue step 5: at most once per second, only for a completed bucket
    if open_positions > 0 and now − ema_ts > COVERAGE_GAP_S: emit! CoverageGap{now − ema_ts, open_positions}   // A69
    obs  = clamp(P_ema, b_lo, b_hi)                          // = median(b_lo, P_ema, b_hi): moves only if ALL obs were on one side
    obs  = clamp(obs, ⌊P_ema·10⁴/(10⁴ + C)⌋, ⌊P_ema·(10⁴ + C)/10⁴⌋)   // C = ema_clamp_bps
    lobs = clamp(L_ema, b_llo, b_lhi) (same clamp)
    dt   = min(now − ema_ts, ema_dtmax_s)
    P_ema = ⌊(P_ema·τ + obs·dt)/(τ + dt)⌋ ;  L_ema likewise with lobs        // τ = EMA_TAU_S = 5 (constant)
    dts  = min(now − slow_ts, liq_slow_dtmax_s) ; L_slow = ⌊(L_slow·liq_slow_tau_s + lobs·dts)/(liq_slow_tau_s + dts)⌋
    ema_ts = slow_ts = now ; ema_count += 1
    (x, y) = reprice(L_ema, P_ema)        // curve: curve_reprice(L_curve, P_ema) = (vT_i, vQ_i)   — cached, decision 3
    ring.push(now, P_ema, L_ema, x, y, b_lo, b_hi, b_llo, b_lhi)      // v0.4.1: bucket extremes, not obs/lobs (R4B-2)
    lb_sec, lb_lo, lb_hi = b_sec, b_lo, b_hi ; clear bucket

rewrite_liquidity(L_new):              // every venue change: graduation, Raydium adoption, fast path — A72, R3B-7 a
    L_ema = L_slow = L_new (graduation) or min(L_new, current) (Raydium) ;
    for every ring entry: l = llo = lhi = L_ema; (x, y) = reprice(L_ema, p)  ;  clear the open bucket
```

- **One-second lag:** an instruction never reads its own observation, so `[pump → liquidate → dump]` cannot feed its own trigger. **Directional aggregation:** one honest observation in the same second neutralises an atomic bundle (+0.00 %) [ECON §5.2]. **Step bounds** (τ = 5 s, C = 100 %): +16.7 % / −8.3 % per 1 s step, +28.6 % / −14.3 % at `dt = 2 s`.
- **The freeze cuts both ways (V2-14, V1-2).** One *contrary* observation per second freezes `P_ema` just as one honest observation neutralises a push; the borrower's paid veto of its own liquidation is priced in ECON §12.9 (Tier A 0.35 SOL/s at the threshold, ≈ 4.6× that 5 % further) and disclosed. With the fixed bound it can no longer widen the exit bound (ECON §12.9 (a)). On Raydium, `L` observations can be moved at zero swap fee by removing and re-adding unlocked LP in quiet seconds; `√(X·Y) ≥ D` bounds the push at `1 − λ ≤ 50 %`, and exit references use `max(L_live, L_ema, L_slow)`.
- **Ring entries** (136 B; v0.4.1): `(ts, p, l, x, y, lo, hi, llo, lhi)`. `p`, `l` feed growth `g` and the expiry reference; the bucket extremes `lo, hi, llo, lhi` feed the settlement box's medians (lower median of the lows, upper median of the highs; v0.4's clamped `obs`, `lobs` let a pushed EMA be frozen into the median, R4B-2); `(x, y)` is the position-independent reprojection that the liquidation EMA leg, the D4 margin and the box's EMA-point amount quote on — computed once per second per market instead of once per position per call (−820 CU per entry per position [V18]). v0.3's `lo` (the Dutch clock's evidence, the ring's own low) is unrelated to v0.4.1's bucket `lo`. The two medians sort 8 values each: ≈ 1–2 k CU per `sol_settle`, unchanged in order from v0.4's one median pair.
- **Reprojection** (ECON §2.1, §2.3): `sp = isqrt⁺(P)`, `X = ⌊L·10⁹/sp⌋`, `Y = ⌈L·sp/10⁹⌉`, so `Y/X ≥ P`; for the curve `curve_reprice` keeps `√(vT·vQ)` and the constant offsets `vT − rT`, `vQ − rQ`, so the EMA leg is the curve itself at `P_i` (with `B̂_str` if `Q > rT_i`). On PumpSwap `Y` is effective (includes VQR).
- **Robust spot** (ECON §5.4): `P_rs = min(P_live, lb_lo)`, valid only if `lb_sec ≥ now − ref_max_age_s`.
- **Sustained growth** (ECON §2.1): per step `⌊(P_i − P_{i−1})·10⁴/(P_{i−1}·Δt_i)⌋` bps/s (i128 intermediate), `g = max(0, min over the last kappa_growth_n steps)`. A single held second produces one step; a genuine trend produces three.
- **Initialisation:** at creation `P_ema = P_spot`, `L_ema = L_slow = √(X·Y)`, one ring entry `(now, P, L, reprice(L, P), P, P, L, L)`. At graduation and at every Raydium adoption `P_ema` carries over and `rewrite_liquidity` runs (above).
- **No observations while the venue cannot execute** (A64, V2-5): `P_ema` stops, the S3 chain runs, and `SolSettle` reports `ema_age_s`. The completed curve of a `PumpMigrating` market is final and still recorded. After `RING_LEN` seconds without an integration the settlement box collapses to the EMA point (§3.8).
- The CPMM `ObservationState` TWAP (100 entries ≥ 15 s apart, atomic bundles cannot move it) is not used in the MVP; a v1.1 candidate second bound [R-RAY §2.6].

### 4.5 Rent, CR, liquidation and settlement predicates

**Rent (A36; ECON §7 v0.4).** Three market-level basis indices, each position's rate curve **and protocol share** locked at open, O(1):

```
k = util_kink_bps (market-level, own version)       U = ⌈ΣB_Q·10⁴/(T + ΣB_Q)⌉ (pre-instruction counters)
accrue(Δt):  P̃ = min(P_ema, P_ACC_MAX) before the roll
             J0 += P̃·Δt ;  J1 += ⌈P̃·min(U, k)·Δt/10⁴⌉ ;  J2 += ⌈P̃·max(U − k, 0)·Δt/10⁴⌉
rent_accrued(pos) = ⌈Q·(base·ΔJ0 + slope1·ΔJ1 + slope2·ΔJ2)/(10⁴·31,536,000·WAD)⌉,   ΔJ = J_now − j_open
R_due = max(rent_accrued, r_min)
lender credit per accrual = ⌊Σ_j A_j·δJ_j/(10⁴·10⁴·31,536,000·WAD)⌋,  A_j = Σ_open Q·coef_j·(10⁴ − rent_protocol_bps_pos)
true-up at repay/settle:   credited_for(pos) = ⌈Q·Σ_j coef_j·ΔJ_j·(10⁴ − rent_protocol_bps_pos)/(10⁴·10⁴·31,536,000·WAD)⌉
                           receivable −= min(credited_for(pos), receivable)                                        (clamp stated, R3A-5)
```

Because each accrual floors the aggregate and each true-up ceils per position, `receivable` never goes negative before the clamp, and it returns to 0 whenever no position is open, whatever governance does to `rent_protocol_bps` (ECON I-P7). The sims now use exactly this form, so the differential test is exact (R3A-3).

**CR and the legs** (ECON §3.3, §5.5, §5.6). `V_eff = V − R_due`; `CR = ⌊V_eff·10⁴/B̂⌋`; `L` = the position's locked `cr_liq_bps`; `K_s = min(settle_keeper_fixed + ⌊V·settle_keeper_bps/10⁴⌋, settle_keeper_cap, V)` (locked terms). Risk quotes use `φ_risk = min(φ_live, venue_fee_hard_cap_bps_pos)`; execution quotes the live fee (§4.3).

```
spot leg : venue_executable ? CR(V_eff, B̂(Q; reprice(L_live, P_rs))) < L : true       (P_rs fresh; SPEC: CR_spot := 0 with no route;
           the route-broken flag and the fee hard cap never make it true — V2-1)
EMA leg  : the newest liq_persist_n ring entries all have CR(V_eff, B̂(Q; x_i, y_i)) < L                  (cached; decision 2)
liquidatable ⇔ (spot leg ∧ EMA leg) ∨ now ≥ deadline
normal buyback allowed ⇔ B̂_live + ert ≤ V − K_s ∧ σ_live ≤ σ_max                          (B̂_live at the live fee: execution)

settlement box (A65):  P_lo = min(P_ema, P_mlo), P_hi = max(P_ema, P_mhi), L_lo = min(L_ema, L_mlo), L_hi = max(L_ema, L_mhi)
                       P_mlo, L_mlo = lower medians of the ring's lo, llo; P_mhi, L_mhi = upper medians of hi, lhi (v0.4.1);
                       the point (P_ema, L_ema) if now − ema_ts > RING_LEN
S1 legs ⇔ B̂_live,risk + ert > V − K_s ∧ B̂(Q; reprice(L_live, P_rs)) + ert > V − K_s ∧ B̂(Q; reprice(L_hi, P_lo)) + ert > V − K_s
S2 legs ⇔ σ_live > σ_max ∧ σ(Q; reprice(L_live, P_rs)) > σ_max ∧ σ(Q; reprice(L_hi, P_lo)) > σ_max          (σ_max locked)
refuted ⇔ B̂(Q; reprice(L_lo, P_hi)) + ert ≤ V − K_s ∧ σ(Q; reprice(min(L_live, L_lo), P_hi)) ≤ σ_max
not liquidatable (disarms) ⇔ ¬expired ∧ ¬EMA leg ∧ CR(V_eff, B̂(Q; reprice(L_lo, P_hi))) ≥ L
kind at execution: S1 if the S1 legs hold, else S2                                         (D3 literal: S1 never capped)
amount (S2, S3): x(P, L) = min(V − K_s, FV(P, L)), FV's p' decided at (P, L) (v0.4.1);
                 settle iff (x(P_hi, L_lo) − x(P_lo, L_hi))·10⁴ ≤ fv_dev_bps·x(P_lo, L_hi);  pay x(P_ema, L_ema)
```

Each predicate is evaluated at the corner that is least favourable to it, so "true" means "true on the whole box". In a sustained uptrend `P_ema ≤ P_mlo` (the EMA lags the median of the last 8 bucket lows), so the S1 corner is `P_ema` itself and honest S1 timing is unchanged; in a ramp's first ≈ 8–11 s, after a step that fills 4 of the 8 entries, or in a steady ramp of ≈ 9.5–16.7 %/s (where the EMA's steady lag is smaller than the median's) the S1 corner is `P_mlo` instead, which delays S1 but never makes it easier (`shortwin-math` `settle_props.rs`); in a genuine reversal `P_ema ≥ P_mhi`, so refutation reads `P_ema` itself (ECON §5.6). Several honest observations at different prices within one second widen the box by that second's range; the keeper therefore observes an S3-due market once per second (§7.1 duty 5). `ert` enters the S1 threshold so no band exists where neither a buyback (budget `V − K_s − ert`) nor S1 is possible. If spot says "cannot buy back" but neither S1 nor S2 is confirmed, the position waits.

**Fair value (D3; ECON §8.6).** Used in S2 and S3 only; S1 is never capped.

```
φ_FV  = min(φ_live, venue_fee_hard_cap_bps_pos)      (A43: an admin fee hike above the hard cap cannot inflate FV)
B̂_FV(P, L) = B̂(Q; reprice(L, P); φ_FV)                  (live fee tier on the hypothetical state, R3B-7 b)
FV(P, L)   = B̂_FV + ⌈p'·B̂_FV/10⁴⌉ + R_due
p'(P, L)   = the locked expiry fee if the position is expired and ⌊V_eff·10⁴/B̂_FV(P, L)⌋ ≥ L, else the locked penalty
             (O15, S2 and S3; v0.4.1 decides it at each evaluated (P, L), never from the raw legs — R4A-3)
stuck migration (S3 on a completed curve): B̂_FV prices the deterministic post-migration pool at P
vesting: lump = x − min(x, ⌊Q·P_ema/WAD⌋ + R_due) vests (§3.3); the rest is immediate (v0.4.1, R4B-6; no B̂_FV reconstruction, R4A-5)
```

**`p'` for expired, not CR-liquidatable positions (R3A-8 a; owner decision O15, SPEC §0, §5.3, §14).** D3 names the "liquidation penalty" in FV; SPEC §4 says an expired position pays "a 2 % expiry fee instead of the full liquidation penalty". The owner recorded the expiry fee for a position settled in **S2 or S3** only because it expired (SPEC §5.3: "the same reasoning applies to S2 as to S3") — the reading under which an S2/S3 of an expired position pays lenders in SOL what a normal expiry would have paid them in tokens. It applies to every S2 and S3 amount, at each box corner and at the EMA point (the sim's `fv_parts`; ECON §2.1, §8.6), which the sims and `shortwin-math` implement. The penalty reading would move 3–8 % of `B̂` from the borrower to lenders (Tier A: 1.82 SOL) and give lenders a reason to prefer S3 over expiry (ECON §8.6).

### 4.6 Size caps (ECON §6.1)

Evaluated in `open_short` only; caps never force closes. `X₀` is the conservative token reserve (ECON §2.1): curve `vT`; PumpSwap `min(X_now, ⌊L_slow·10⁹/isqrt⁺(P_ema)⌋)`; Raydium additionally `≤ ⌊D·10⁹·10⁴/(λ_min_bps·isqrt⁺(P_ema))⌋`, the locked depth divided by `λ_min` (V2-8). `X₁ = X₀ + q`. `φ` is the one-way venue fee in **ppm** (§4.2).

- Per position (SPEC): `q ≤ X₀·(√(1 + ι_pos) − 1)` → 2.470 % of `X₀` (Tier D 1.489 %); integer form (the sim's, normative): `q ≤ isqrt(⌊X₀²·(10⁴ + ι_pos)/10⁴⌋) − X₀`.
- Per position, fee-scaled (ECON N2): `q ≤ ⌊fee_size_cap_x100·φ·X₀/(100·10⁶)⌋` → 3.0·φ·X₀; binds below φ ≈ 0.82 % (Raydium 25 bps pools: 0.75 % of `X₀`).
- Per market (SPEC, post-open incl. the new q): `ΣQ + q ≤ X₁·(1 − 1/√(1 + ι_mkt))` → 8.713 % of `X₁` (Tier D 4.654 %); integer form (the sim's, normative): `ΣQ + q ≤ X₁ − isqrt(⌊X₁²·10⁴/(10⁴ + ι_mkt)⌋)`, which can exceed the real-valued bound by one raw unit.
- Per market, fee-scaled: `ΣQ + q ≤ ⌊fee_oi_cap_x100·φ·X₁/(100·10⁶)⌋` → 7.5·φ·X₁; binds below φ ≈ 1.16 %.
- Utilisation after the open ≤ `util_cap_bps` (90 %).
- Per wallet (ADOPTED Q10): `exposure.q_open + q ≤ wallet_cap_bps·OI_cap/10⁴`, `OI_cap` = the binding per-market cap.
- Tier D graduation guard: `ΣQ + q ≤ γ·(rT + q)`, γ = `curve_oi_gamma_bps` (50 %).
- `N_open ≥ min_notional`; `Σ N_open ≤ market_max_notional` (beta 25 SOL, ADOPTED Q21).

### 4.7 Tiers, USD thresholds and pool age

```
tier::assign:
  PumpCurve | PumpMigrating                                  -> D
  PumpSwap | RaydiumV4 | RaydiumCpmm:
     R_slow = Y(reprice(L_slow, P_ema)) − VQR_live  (≥ 0; VQR = 0 on Raydium)
     R_eff  = min(R_now, R_slow)  ;  Raydium: R_eff = min(R_eff, ⌊⌊D·isqrt(P_ema)/10⁹⌋·10⁴/λ_min_bps⌋)   (locked SOL / λ_min; V2-8)
     Λ      = 2·R_eff                                   [lamports]   (ADOPTED Q19: real SOL only, no VQR/boost)
     age    = now − graduated_at                                      (ADOPTED Q18; A12)
     if now − oracle.publish_time > oracle_stale_s             -> C  (ADOPTED Q8: fail safe; opens allowed at C terms)
     θ_x_lamports = θ_x_usd_e6·10⁹/(price·10^(expo+6))
     if age < AGE_C_S (24 h) or Λ < θ_B                          -> C   (AGE_C_S, AGE_A_S: SPEC constants, §2.3.3)
     elif Λ ≥ θ_A and age ≥ AGE_A_S (7 d)                        -> A
     else                                                         -> B
  None -> no opens
```

- **Real reserves** (ADOPTED Q19): PumpSwap graduates carry `virtual_quote_reserves = ⌊0.2069·migrated SOL⌋` (proportional, not a fixed 17.58 SOL; confirmed on 130 of 130 non-mayhem migrations, rule inferred from data, source closed — UNVERIFIED as a rule; C1-13) backed by a boost vault that pump spends on buy-and-burn within ≈ 6 minutes [R-VEN §1.4, Review corrections 3]. Counting it would inflate liquidity. Raydium `R` excludes `need_take_pnl` / fee counters (§4.1).
- **Age** (ADOPTED Q18, A12): `graduated_at` is the first on-chain observation of the pool (PumpSwap), the listing time (AMM v4: `pool_open_time` is unusable — unclamped, admin-resettable, 0 on pump graduation pools), or for CPMM `min(now, max(open_time, T_CLAMP))`, a valid lower bound because `open_time` has been clamped above creation time since commit `a7330e3` (2024-05-16) and is immutable afterwards [R-RAY §7]. It persists across Raydium pool switches. Owner item O13 (extended by R3B-12): for a market listed after graduation the age counts from listing on AMM v4, on PumpSwap and on CPMM pools whose `open_time` precedes `T_CLAMP`, which keeps such markets at Tier C for 24 h and at most Tier B until day 7; markets created before graduation observe the transition live.
- **Oracle** (ADOPTED Q8): Pyth sponsored shard-0 SOL/USD `7UVimffx…` (owner `rec5EKMG…`, feed `ef0d8b6f…`), read only by `crank_sol_usd` and cached in `OracleCache`, which `open_short` and `crank_price` read. Its heartbeat is 55 s (or a 0.5 % move); publish-time steps of 52–54 s and staleness up to ≈ 45 s were measured, hence `oracle_max_age_s = 120` (C1-12) [R-VEN Review corrections 2]. **Closes, liquidations and settlements never read the oracle** (I10).
- **Examples** (SOL ≈ $121): $500 k ≈ 4,124 SOL of Λ, $100 k ≈ 825 SOL. A fresh pump graduate has ≈ 67 SOL of real quote (Λ ≈ 135 SOL) → Tier C. Fartcoin's AMM v4 pool (38,100 SOL real) is Tier A by liquidity but Tier C for its first 24 h on a new market and Tier B until day 7, because AMM v4 age starts at listing (a long-graduated PumpSwap pool behaves the same way, O13); USELESS on CPMM (`open_time` 2025-05-10) is Tier A at once [R-RAY §6.4].

---

## 5. Liquidation and settlement paths

### 5.1 Keeper and program decision tree (per position, every second near a threshold)

```
if market.venue == PumpMigrating:
    keeper : pump migrate_v2 top-level in its own transaction (A15); then crank_price → PumpSwap
    program: liquidate / expire / S1 / S2 revert VenueMigrating; S3 only after max_migration_pause_s (§5.3)
elif !venue_executable(market):                          # halted pool, admin disable, vault below counters, reader failure
    keeper : crank_price every ≤ 30 s (continuous S3 chain); Raydium: after pool_fail ≥ 120 s, propose the deepest
             eligible candidate and execute the fast path when its 60 s window closes (§2.6)
    program: sol_settle(S3) once the timer allows and the position is liquidatable or expired;
             healthy positions stay frozen (close impossible) until the route returns or their deadline (§5.3)
elif s3_no_route(market):                                 # executable venue, but route-broken flag or fee > global hard cap
    keeper : as usual (closes, liquidations, expiries and settlements run where their bounds allow);
             crank every ≤ 30 s so the S3 chain stays continuous; fee > hard cap also feeds pool_fail on Raydium
    program: sol_settle(S3) for positions liquidatable (spot leg evaluated normally) or expired, once the timer allows;
             the amount must be box-consistent
elif !liquidatable(pos) and !armed(pos):                  # nothing to do; keep observing (ECON §5.8)
elif curve live && q > rT:                                # straddle: ≥ 3-transaction Jito bundle with complete_curve (§5.4)
elif lb stale (lb_sec < now − ref_max_age_s):             # crank_price this second, act next second
elif normal buyback allowed (B̂_live + ert ≤ V − K_s ∧ σ_live ≤ σ_max):
    if now ≥ deadline && !cr_liquidatable:  expire()      # when B̂_live ≤ max_in (expiry reference, δ_exp); else retry next second
    else:                                   liquidate()   # when B̂_live ≤ max_in (fixed δ_liq = 2 %), via Jito bundle; retry on ExitBoundNotMet
else:                                                     # S1/S2 legs hold, or the position is armed: sol_settle every second;
                                                          #   it arms, keeps the clock, disarms only on box-robust evidence,
                                                          #   settles (kind chosen at execution) or waits for a consistent amount
```

A keeper's `liquidate`, `expire` or `sol_settle` for a market is that second's observation for the market; it sends no separate crank (§7.1).

### 5.2 Waterfalls

These are ECON §8 exactly. Each is a pure `math::waterfall` function whose legs are non-negative and sum to `V` (property-tested, ECON I-E4). All keeper, split, fee and rent-share terms are the position's **locked** values. `B` is the measured spend; `r = min(R_due, V − B)`; `base = min(B̂_live,risk, B̂_ref)` (quotes, never the measured spend).

| Path | Order of payment from `V` |
|---|---|
| **close** | 1. `B` (`max_in = min(max_buyback_cost, V − R − ert)`). 2. `R_due`: `⌊R·rent_protocol_bps_pos/10⁴⌋` to protocol, the rest to lenders. 3. Remainder to the borrower |
| **liquidate** | 1. `B` (`max_in` of §3.6). 2. `r` (split as in close). 3. Penalty `π = min(⌈p·base/10⁴⌉, V − B − r)`: keeper `⌊π·keeper_split/10⁴⌋`, protocol `⌊π·protocol_split/10⁴⌋`, lenders the rest (vests). 4. **Keeper floor** `top = min(max(0, K_s − ⌊π·keeper_split/10⁴⌋), V − B − r − π)` to the keeper, paid by the borrower (A74; R3B-10). 5. Remainder to the borrower |
| **expire** | 1. `B` (expiry `max_in`). 2. `r`. 3. Expiry fee `e = min(⌈expiry_fee_bps·base/10⁴⌉, V − B − r)`, 100 % lenders (vests). 4. **Keeper** `K_e = min(expiry_keeper_fixed + ⌊expiry_keeper_bps·B̂_ref/10⁴⌋, expiry_keeper_cap, V − B − r − e)`. 5. Remainder to the borrower |
| **settle** | 1. `K_s` to the keeper. 2. Lenders `x`: S1 `V − K_s`; S2/S3 `x(P_ema, L_ema) = min(V − K_s, FV)` if box-consistent (§4.5); of an S2/S3 take, `x − min(x, ⌊Q·P_ema/WAD⌋ + R_due)` vests. 3. `V − K_s − x` to the borrower. The protocol gets nothing (SPEC §6.3) |

- **Senior in the bound: an expiry's rent and fee, not a liquidation's** (C0-1 (3); v0.4.1, R4A-1, R4B-1). An expiry's buyback may exceed `V − R − π_ref − ert` only if the fair reference cost itself does (`max(V − R − π_ref − ert, B̂_ref·(1 + ε))`, ECON §8.2), so a sandwich cannot truncate rent or expiry fee wherever the reference leaves room (I-E2). A liquidation reserves nothing (§3.6): a reserve there blocks honest liquidations in fast pumps; a sandwich or held attacker can take rent and penalty up to the bound's slack (I-E3; 22 of 709 exits in the exit fuzz, ECON §13), and the held model's larger truncation on aged positions is disclosed (§8.5, O19). The payment order of SPEC §5.2 is unchanged.
- **Keeper floor** (A74; N22, owner visibility). The SPEC keeper share of a small liquidation is below the settlement reward `K_s`, so a third-party keeper would rather let a small position gap into S1 (a lender loss). The top-up closes that; it is zero above the crossover vaults (≈ 5.0 SOL Tier A, 1.8 B, 1.7 C, 1.4 D) and never exceeds what the borrower would lose in S1 (ECON §8.5).
- Lender credits go to `market_vault` (`vault.lender_sol += …`) through `pool::repay` / `pool::settle` (rent true-up against `receivable`; a negative Δ is a shortfall debit; lumps vest). Protocol credits go to `vault.protocol_fees`.
- **A18: the borrower is paid through the position close.** The borrower remainder is never pushed as a separate transfer; it stays on the `ShortPosition` account, which is closed to `borrower`, so the borrower also recovers the account rent and the credit is always ≥ the rent-exempt minimum (a 0-lamport borrower cannot block its own liquidation) [V4; C0 verified].

### 5.3 SOL settlement conditions, routes and the S3 timer

- **S1 / S2** are as in §3.8 and §4.5: triple-leg (live, robust spot, and the settlement box), S1 whenever its legs hold (D3), and persistence (one eligibility clock held for `settle_persist_n` EMA updates, so ≥ 39 s, reset only by box-robust evidence).
- **Three route predicates, each with one job** (A23 as amended, A50, A71; C0-4, C0-10, V2-1, R3A-4, R3B-8):
  - **`venue_executable`** = the venue predicate in the table below (and all venue-state accounts parse, A63). It alone drives `B̂_live`, closes, liquidations, expiries, the "no route ⇒ spot leg true" rule and observation recording (A64).
  - **`s3_no_route`** = `!venue_executable ∨ market.route_broken ∨ venue_route_broken[venue] ∨ φ_live > config.venue_fee_hard_cap_bps`. It feeds only the S3 chain, `sol_settle`'s "no route now" and the open gate.
  - **`pool_fail`** = `!venue_executable ∨ φ_live > config.venue_fee_hard_cap_bps` (Raydium). It feeds only the fast-path chain `pool_fail_since` / `last_pool_fail_obs` (§2.6). It never reads the Emergency flag.
  - The market-level predicates use the **global** hard cap; every risk quote and FV use the position's locked value (R3A-4 c; A26).
  - T-S3-BROKEN asserts that with the flag set the set of liquidatable positions, every close outcome and the `pool_fail` chain equal those of the unflagged run, and that the flag only adds S3 for positions liquidatable (spot leg evaluated normally) or expired.

  The venue predicates:

| Venue | Venue predicate | Source |
|---|---|---|
| PumpCurve | `!curve.complete` | [R-PUMP §7] |
| PumpMigrating | false | SPEC §5.3 |
| PumpSwap | canonical pool valid ∧ `pool_base_vault.amount > 0` ∧ `!(disable_flags & PAMM_BUY_DISABLED_BIT)` where the bit is a **compile-time constant set from test gate T-DISABLE** (a hard gate before M6 closes). Until then, and for any unknown non-zero flag afterwards, `disable_flags ≠ 0` blocks opens and alerts but is **not** S3 evidence; a real halt reaches S3 through the Emergency route-broken flag (A24; C0-10) | [R-VEN §7; V6 of v0.1] |
| RaydiumV4 | `(status ∈ {1, 6} ∨ (status == 7 ∧ now ≥ pool_open_time))` ∧ `coin_vault ≥ need_take_pnl_coin` ∧ `pc_vault ≥ need_take_pnl_pc`. Status values above 7 revert in the venue, so they are "no route" | [R-RAY §5.3] |
| RaydiumCpmm | `(status & 0b100) == 0 ∧ now ≥ open_time` ∧ `vault_i ≥ protocol_i + fund_i + creator_i` for both sides | [R-RAY §5.3] |
| None | false | — |

  A "no route" that exists only for one position (`Q ≥ X`) is S2 territory, not S3. The Raydium predicates come from source, not from guesses [R-RAY §5.3]; today none of 705,975 AMM v4 pools and 236,265 CPMM pools is swap-disabled.
- **Venue interface breakage (C0-4 (d); V1-8, V2-6).** If a venue changes its interface so that our CPI fails, every exit reverts and the failure cannot be recorded on-chain. The keeper detects it in simulation, alerts, and pages the Emergency multisig, which publishes an extras template set (additive change, including new per-mint or per-pool PDAs) or (once O6 is recorded) sets the route-broken flag, which starts the S3 chain. If the change breaks the layout of a venue-*state* account instead, `crank_price` and `sol_settle` see "not executable" (A63) and the chain starts by itself. Test gates: T-REPLAY-0428 replays pump's 2026-04-28 account change against the exit paths with extras configured; T-REPLAY-PDA adds a synthetic "new per-mint PDA remaining account" change and recovers it with a template; T-S3-BROKEN checks that a flagged route reaches S3 after `s3_wait_s` and changes nothing else.
- **Liveness without a route (V2-5; disclose).** While the venue cannot execute — an AMM v4 admin halt (status 2–5, or 7 with a future open time), a CPMM status bit, a venue interface break — **no position can close**, **liquidatable positions reach S3 after the chain**, and **healthy positions stay frozen until the route returns or their deadline** (up to 30 days for Tier A): S3 requires "liquidatable" (SPEC §5.3), the halted pool is no longer observed, so the EMA leg cannot fire from price moves elsewhere. At the deadline the position is expired and liquidatable, S3 applies and FV is priced at the last pre-halt `P_ema` (the event reports its age); if the token rose elsewhere meanwhile, a borrower who would have been underwater can still receive `V − K_s − FV`. The Raydium fast path helps only if another eligible pool exists: of Fartcoin's nine Raydium SOL pools only the adopted `Bzc9NZfM…` (38,176 SOL real, λ 0.9978) is eligible [V15], so one 2-of-3 admin action [V14; R-RAY §5.1] freezes every Fartcoin position until its deadline. Options that shorten the bound are owner decisions (§10, O14); none is in the MVP default.
- **Continuous timer (C0-16; ECON §10.3).** Every prologue observes `s3_no_route`. On a no-route observation: if `no_route_since == 0` or `now − last_no_route_obs > S3_OBS_GAP_S` (60 s, a constant), set `no_route_since = now`; always set `last_no_route_obs = now`. Any observation with a route clears both. S3 is allowed iff the position is liquidatable (spot and EMA legs; the spot leg is true without evaluation only if the venue is not executable) or expired, `s3_no_route` holds **now**, the chain is unbroken, and

  ```
  now ≥ max(no_route_since, graduated_at + grad_grace_s, migration_seen_at + max_migration_pause_s·[venue == PumpMigrating]) + s3_wait_s
                                                                     (the position's locked grad_grace_s, max_migration_pause_s, s3_wait_s)
  ```

  The in-house keeper cranks every market without a route at least every 30 s (§7.1). A lender-keeper that cranks only during outages can no longer add up separate outages (ECON §10.3 self-test: S3 moves from 600 s to 1,020 s).
- **Graduation in progress** (SPEC §5.3): all liquidation-class instructions pause while `PumpMigrating`; rent accrues and the EMA keeps integrating the completed curve's price. Anyone (including our keeper) can call `migrate_v2`, so the pause is normally 0–6 slots [R-PUMP §7.4]. If migration never completes, S3 becomes possible `max_migration_pause_s + s3_wait_s` (70 min) after `migration_seen_at` (ADOPTED Q5). After graduation, S3 cannot start until `graduated_at + grad_grace_s`; the graduation rewrites the ring's liquidity (A72), so the settlement box is not widened by the curve's larger `√(vT·vQ)`.

### 5.4 Graduation-specific cases

1. **Buyback completes the curve** (`q == rT`): allowed; the venue becomes `PumpMigrating` after the leg (ECON §10.2).
2. **Straddle** (curve live, `q > rT`; A14, ADOPTED Q14, C1-1, R3B-11). Pump rejects any buy above `rT` (6021) and never fills partially [R-PUMP §7.3]. The MVP has no in-program two-leg execution. The keeper (or the borrower's client) sends a **Jito bundle of at least 3 transactions** (at most 5 per bundle, sequential, all-or-nothing, tip required [R-PUMP Review corrections 1]):
   - **tx1**: ComputeBudget ×2 + **`complete_curve(max_sol_cost)`** of the helper program (A66): it reads `rT` from the bonding curve **at execution**, and if the curve is not yet complete it CPIs pump `buy_v2(rT_live, max_sol_cost)` with the transaction's own signer as `user`; if the curve is already complete it succeeds as a no-op. v0.3's tx1 named `rT` as read before the bundle, so any curve trade that landed in between made pump reject the buy (6021) or left the curve incomplete for `migrate_v2`, and the bundle reverted while the price ran (R3B-11). The helper is a separate, stateless program (≈ 150 lines, no accounts of its own, audited with the core, Governance upgrade authority) because it forwards a user's signature to a venue, which the core program never does (A4). Stack: helper 1 → pump 2 → System/Token/pfee 3; ≈ 11 trace entries.
   - **tx2**: ComputeBudget ×2 + `migrate_v2` top-level, with its 2 remaining boost accounts (51 + 2 = 53 entries: nothing else fits under 64); `migrate_v2` is idempotent.
   - **tx3**: ComputeBudget ×2 + pAMM `init_user_volume_accumulator(payer = keeper, user = trader)` if needed + `crank_price` (transition) + `liquidate` / `close_short` on PumpSwap (≈ 2 + 3 + 2 + 17 ≈ 24 entries).
   - Pricing uses `B̂_str` on-chain (§4.3, V1-9): the position buys all of `Q` on the post-migration pool, which is exactly tx3, so CR, S1, S2 and the exit bounds describe the path that executes. The γ = 50 % open cap makes straddles rare (ECON §10.2). Completing the curve is self-financing for the keeper (it keeps the `rT` tokens and can sell them after the +45.66 % boost push) [ECON §10.1].
   - **Keeper fallback** (V1-9): if tx3 would fail (its simulation returns `UseSolSettle`, `ExitBoundNotMet` or `InsufficientVault`), the keeper sends tx1 + tx2 only and then treats the position as an ordinary PumpSwap position in the following seconds (crank, liquidate or `sol_settle`). The bundle is not yet simulated (**UNVERIFIED**, gate T-STRADDLE, which now includes "a curve trade lands between the keeper's read and the bundle").
   - A `migrate_v2` CPI from inside short.win is rejected: the top-level call already reaches stack height 4, so wrapping it would hit the maximum of 5 (A15) [V2 of v0.1; R-PUMP Corrections 3].
   - v1.1 option: an in-program `ExitInProgress` state that buys `rT`, records the partial buyback and finishes on PumpSwap in a later instruction.
3. **Boost window** (A13): opens are refused while `boost_vault.amount > boost_dust_lamports ∧ now < graduated_at + boost_window_s`. The boost is proportional (20.69 % of migrated SOL) and pushes the price ×1.4566 within ≈ 6 minutes; positions opened late on the curve that survive graduation are liquidated normally by that push [ECON §10.1]. There is no extra liquidation grace.
4. **Creator changes mid-position:** `creator_vault` is re-derived from the live curve or pool on every trade [R-PUMP §8].

### 5.5 Venue admin powers and upgrades (exposure and mitigations)

| Venue | Who | Power relevant to open positions | Mitigation in this design |
|---|---|---|---|
| pump, PumpSwap, pump-fees | upgrade authority `7gZuf…` (all three) | Arbitrary code change; 4 pump upgrades in Sept 2026, the 09-15 and 09-23 ones without public changelog [R-PUMP §1] | Opens refused once the `VenueGuard` latch records a deploy-slot change, until re-validated (ADOPTED Q13, A7); exits run on the compiled adapter; bounded extras for additive account changes; route-broken → S3 for anything else |
| PumpSwap | `GlobalConfig.admin` `FFWtrEQ4…` | Global buy/sell halt via `disable_flags`; fee changes via pfee (no timelock) [R-VEN §7] | A24; `venue_fee_cap_bps` on opens; `φ > venue_fee_hard_cap_bps` = S3 evidence only; risk quotes and FV capped at the hard cap (A43, V2-1) |
| Raydium AMM v4 + CPMM | upgrade: Squads v4 vault `FytDrVz…`, **3 of 4, no timelock**; last deploys 2026-09-10; 6 AMM v4 and 10 CPMM upgrades since 2025; **no verified builds** (behavioural match to `raydium-amm@d26944b` / `raydium-cp-swap@59fb845` only) [R-RAY §1.1–1.3] | Arbitrary code change | Same guard, extras (AMM v4 v2 and Anchor CPMM accept trailing extras [R-RAY §9.6]), route-broken |
| Raydium AMM v4 | pool admin `GThUX1…` = Squads v3, **2 of 3, no timelock** (hard-coded as `config_feature::amm_owner` for mainnet builds [V14]) | Per pool: disable swaps (status 2–5, or 7 with a future open time); set the swap fee to anything below 100 %, immediately [R-RAY §5.1] | Exact venue predicate (§5.3); fee caps in risk quotes (A43, V2-1); fast pool switch on `pool_fail` (§2.6); S3 with FV cap; freeze of healthy positions disclosed (§5.3, O14) |
| Raydium CPMM | same `GThUX1…` | Per pool: status bit 2 disables swaps. Per **config**: trade and creator fee up to 99.9999 % **for every pool on that config at once** [R-RAY §5.2] | Same. `open_time` cannot be changed after initialisation, so the age bound is safe |
| Raydium (both) | `pnl_owner`, `protocol_owner`, `fund_owner`, `collect_lamports` | Fee and PnL collection, excess-lamport collection | Reserve-neutral by construction (counters are excluded from pricing reserves); no instruction moves principal or closes a pool [R-RAY §5] |

An admin fee hike between `venue_fee_cap_bps` and `venue_fee_hard_cap_bps` makes exits more expensive for borrowers (the venue charges it) but cannot inflate FV beyond the hard cap. This split of the value transfer is part of owner item O18 (§10); the alternative is FV at `min(current fee, fee at open)` [R-RAY §9.4].

---

## 6. Compute, instruction-trace and transaction-size budget

The math figures below are **measured** primitives of the current `shortwin-math` crate on SBF v3, multiplied by the number of calls each v0.4 procedure makes [V18]. Runtime and account-handling overhead is an **estimate** (UNVERIFIED) built from Agave's fixed costs [V19]. Venue CPI costs are measured on mainnet (research notes). Everything is replaced by Mollusk/LiteSVM measurements under the mainnet feature set (p-token) in the milestones that build each path; CI fails if a measured value exceeds the limit column (T-CU, T-CU-MATH, T-TRACE, T-SIZE).

### 6.1 Compute units

**Measured primitives** (sbpfv3, platform-tools v1.57, `shortwin-math` as built for M0 against v0.3; per call, loop overhead subtracted; Tier A PumpSwap magnitudes) [V18]:

| Primitive | CU | Primitive | CU |
|---|---|---|---|
| `mul_div` u128 fast path | 691 | `mul_div` 256-bit fallback (Knuth D) | 783 |
| `mul_div_u64` (via u128 division) | 267 | native u64 `a·b` then `div_ceil` | 76 |
| PumpSwap `buy_exact_out` (3 fee components) | 1,452 | CPMM / AMM v4 `buy_exact_out` | 1,872 / 959 |
| `reprice(L, P)` | 843 | `isqrt(X·Y)` u128 / via U256 | 734 / 624 |
| `rent::due` / `rent::credited_for` / `rent::accrue` | 1,287 / 2,637 / 1,109 | `record` + `integrate` (one EMA roll) | 2,990 |
| `obs_median` (8 + 8 values; v0.4.1 sorts 4 × 8 bucket extremes ≈ 3.2 k, by scaling) | 1,604 | `sigma_xy` | 1,473 |
| **EMA-leg entry, v0.3** (reprice + quote + CR) | **2,289** | **EMA-leg entry, v0.4** (cached `(x, y)` + quote + CR) | **1,468** |

Two consequences. First, R3B-3's projection of ≈ 0.9–1.0 M CU of math per liquidation assumed a bit-by-bit 256-bit divider (≈ 8.8 k CU per `mul_div` in the reviewer's bench); the crate never used one — it takes the u128 path first and falls back to Knuth D, which costs only 13 % more — so that projection does not apply. Second, what v0.4 removes is real but modest: caching the reprojection saves ≈ 820 CU per ring entry per position, and dropping the executable-run scan removes up to 8 more entries, so a worst-case liquidation's math falls from ≈ 67 k to ≈ 35 k CU.

**Math per instruction** (PumpSwap quotes, the costliest family on the pump path; `docs/design/arch/cu_budget.py` reproduces the numbers): prologue with a roll ≈ 11 k (accrue, vest release, lender credit, roll with the cached reprojection, spot price, `√(X·Y)`, fee tier); `open_short` ≈ 24 k; `close_short` ≈ 18 k; `liquidate` ≈ 35 k (v0.3 worst case ≈ 67 k); `expire` ≈ 32 k; `sol_settle` (armed, S2/S3 amount path: legs, box, refutation, three amount corners, pool settle) ≈ 46 k (v0.4.1: +1.6 k for the second pair of medians; `p'` per corner is one CR comparison each); `crank_price` ≈ 13 k; `deposit` / `withdraw` ≈ 12 k (`withdraw` + ≤ 1 k for `cal_advance`, which touches 0–1 buckets per day elapsed); `claim_sol` ≈ 3 k.

| Instruction | short.win math (measured × calls) | runtime, accounts, own CPIs (est.) | Venue CPI (measured) | Total (est.) | Tx CU limit (PROPOSED) |
|---|---|---|---|---|---|
| `open_short` curve | 24 k | 20–35 k: Anchor account checks, 2–3 PDA checks (1,500 each), 5 System CPIs (946 + callee each), `transfer_checked`, event | `sell_v2` 81,628–84,498 [R-PUMP §6.6] | 126–144 k | 250 k |
| `open_short` PumpSwap | 24 k | 25–40 k (+ WSOL lifecycle) | `sell` 74,157 [R-VEN §3.4] | 123–138 k | 250 k |
| `open_short` RaydiumV4 / CPMM | 24 k | 20–35 k (+ approve / WSOL) | 15,204–15,491 / 24,147–33,713 [R-RAY §4.3] | 59–93 k | 150 k |
| `close_short` curve / PumpSwap | 18 k | 20–40 k | `buy_v2` 89,448 (96,871 incl. UVA creation) / `buy` 85,114 | 123–155 k | 250 k |
| `liquidate` / `expire` curve | 35 k / 32 k | 20–35 k | `buy_v2` ≈ 89–97 k | 141–167 k | 300 k |
| `liquidate` / `expire` PumpSwap | 35 k / 32 k | 25–40 k | `buy` 85,114 | 142–160 k | 300 k |
| `liquidate` / `expire` Raydium | 35 k (CPMM quotes +3 k) | 20–35 k | ≈ 15.4 k / 24–34 k | 70–107 k | 200 k |
| `sol_settle` | 46 k | 10–20 k (+ ≈ 5 k when it creates a `ShareEpoch`) | — | 56–71 k | 150 k |
| `crank_price` | 13 k | 5–10 k | — | 18–23 k | 60 k |
| `crank_price` × 6 in one v1 transaction (§6.4, §7.1) | 6 × 13 k | 6 × 5–10 k | — | 110–140 k | 250 k |
| `propose_pool` / `execute_pool_switch` | ≈ 8 k (two λ and `D`, eligibility) + ≈ 7 k for the ring rewrite on adoption | 10–20 k | — | 25–35 k | 100 k |
| `flag_venue_upgraded` / `revalidate_venue` | < 1 k | copy of the ProgramData into the input region (10 MiB for pump / pAMM) — **UNVERIFIED** (U5) | — | UNVERIFIED | 400 k |
| `deposit` / `withdraw` | 12 k | 15–30 k (T22 `transfer_checked`, epoch records) | — | 27–42 k | 100 k |
| `claim_sol` / `cover_deficit` / `sync_lender` | 3 k (+ 3 k per epoch converted) | 5–10 k | — | 8–20 k | 60 k |
| `create_market` | ≈ 10 k | 60–110 k (4 inits, ATA create CPI, UVA init CPI, filter, Raydium verification) | — | 70–120 k | 250 k |
| straddle bundle (§5.4) | tx1 `complete_curve` < 2 k + 3–5 k | | tx1 `buy_v2` ≈ 89–97 k; tx2 322,371–358,072 [ECON T7/T8]; tx3 ≈ 150–200 k | | each ≤ 1.4 M |

- Clients always prepend `SetComputeUnitLimit` = 1.1 × the simulated value; keepers use exactly that (C0-17, §6.6) [R-JUP §2.2].
- **T-CU-MATH (M0b gate):** a Mollusk bench of every primitive above, with a budget of the measured value + 20 %; a regression fails CI. **T-CU** then measures whole instructions.
- ECON §5.8's keeper cost model takes 35 k / 90 k / 220 k CU for a crank / `sol_settle` / `liquidate`; these v0.4.1 estimates are lower (18–23 k / 56–71 k / 141–167 k), so ECON's figures are an upper bound until T-CU replaces both.

### 6.2 Instruction trace (64 entries per transaction; C1-1)

`MAX_INSTRUCTION_TRACE_LENGTH = 64` counts every top-level instruction (ComputeBudget included) plus every CPI at any depth [agave@f7716596 `transaction-context/src/lib.rs` L26, `src/transaction.rs` L493–494; R-PUMP §6.9]. Per short.win instruction (leg counts from §2.4.1, + event 1, + open's margin and fee transfers 2, + normalisation 0–3):

| Instruction | Curve | PumpSwap | Raydium v4 / CPMM |
|---|---|---|---|
| `open_short` | 9 + 1 + 2 = **12** (≤ 15) | 17 + 1 + 2 = **20** (≤ 23) | 10 + 1 + 2 = **13** (≤ 15) |
| `close_short` / `liquidate` / `expire` | 11 + 1 = **12** (≤ 15) | 15 + 1 = **16** (≤ 19) | 8 + 1 = **9** (≤ 11) |
| `sol_settle` | 2 (+3 when it creates a `ShareEpoch` record) | same | same |
| `crank_price`, `propose_pool`, lender ixs, `flag_venue_upgraded` | 2 | 2 | 2 |
| `claim_venue_rewards` | ≈ 5 | ≈ 9 | n/a |
| `complete_curve` (straddle tx1, helper program) | ≈ 1 + 8 + 2 (ComputeBudget) = **11** | — | — |

A keeper transaction `ComputeBudget ×2 + crank_price + liquidate` stays ≤ 2 + 2 + 19 = 23 entries; a batch of `k` cranks uses `2 + 2k` entries (k = 6: 14). The straddle counts are in §5.4; `migrate_v2` must be alone with its ComputeBudget instructions. T-TRACE counts every one of these in LiteSVM and fails CI above the table.

### 6.3 Stack depth (C1-8)

`MAX_INSTRUCTION_STACK_DEPTH = 5` (SIMD-0268 not active on mainnet) [R-RAY §4.3; C1-8]. short.win at height 1 → venue 2 → Token / System / pfee / event self-CPI 3. Pump's own CPIs sit exactly one level below pump [R-PUMP §6.7]; Raydium never creates accounts, so it always peaks at 3 [R-RAY §4.3]. PumpSwap `buy` may create missing fee ATAs through the ATA program (**UNVERIFIED**), which would reach 4; the existence requirement of §3.12 prevents that path, so every leg peaks at 3 and **short.win keeps 2 spare levels** (it can itself be CPI'd once, e.g. by a Squads vault transaction). Without the existence requirement the headroom would be ≥ 1 level. short.win's own System/Token CPIs are at height 2. T-DEPTH runs every venue path through one wrapper level and checks that a missing fee ATA fails cleanly with `VenueAccountMismatch`.

### 6.4 Transaction size and account locks

Limits: 1,232 B for legacy/v0 transactions; 64 accounts per transaction (the lock-limit increase is not activated); v1 transactions (4,096 B, no ALTs, 64 inline addresses) are live since slot 447,120,000 [R-JUP §3.1; C1 verified].

**A28:** v0 + one **protocol ALT** holding the static accounts: programs (pump, pAMM, pfee, AMM v4, CPMM, Token, T22, ATA, System, ComputeBudget is top-level and stays static, short.win event authority); WSOL mint; pump `global`, `event_authority`, `fee_config`, GVA; pAMM `global_config`, `event_authority`, `fee_config`, GVA; the AMM v4 and CPMM authorities; the 21 CPMM `AmmConfig`s; `config`, `venue_guard`, `oracle_cache`; the 8 + 8 pump and 8 + 8 PumpSwap fee/buyback recipients and their WSOL ATAs. The venue ProgramData accounts left the ALT: only `flag_venue_upgraded` and `revalidate_venue` read them. Pump's own ALT `Hyif6eWb…` may be combined [R-PUMP §6.8]. Per-market accounts stay inline; top-level program ids must be static keys.

| Tx | Unique accounts (est.) | Inline keys (with ALT) | Estimated size |
|---|---|---|---|
| `open_short` curve | ≈ 40 | ≈ 21 | ≈ 900–980 B |
| exits curve | ≈ 38 | ≈ 20 | ≈ 870–950 B |
| `open_short` PumpSwap | ≈ 43 | ≈ 22 | ≈ 930–1,010 B |
| exits PumpSwap | ≈ 42 | ≈ 22 | ≈ 920–1,000 B |
| `open_short` / exits RaydiumV4 | ≈ 24 | ≈ 14 | ≈ 650–720 B (≈ 950 B even without the ALT) |
| `open_short` / exits RaydiumCpmm | ≈ 27 | ≈ 15 | ≈ 690–760 B |
| `sol_settle` / `crank_price` | ≈ 15 | ≈ 8 | ≈ 450 B |

These are estimates (**UNVERIFIED**; T-SIZE). If a pump-venue worst case exceeds 1,232 B: a per-market ALT created by the keeper at graduation, or v1 transactions for keepers (and for borrowers whose wallet supports them) [R-JUP §3.1].

**Crank batches (v0.4.1, R4B-9).** A28 keeps per-market accounts inline, so batching `crank_price` fills a v0 transaction quickly: ≈ 209 B of signature, header, static keys, blockhash and ALT section, plus ≈ 213 B per PumpSwap / CPMM crank (6 inline keys) or ≈ 180 B per curve crank (5). v0.4's "≈ 6 per transaction" does not fit; **keeper batches are v1 transactions**, where every key is inline (64 addresses) and 6 cranks take ≈ 42 keys and ≈ 1.6 kB (`sim/keeper_cost.py`):

| k | v0 PumpSwap / CPMM | v0 curve | v1 (all keys inline) |
|---|---|---|---|
| 3 | 848 | 749 | 953 B, 24 keys |
| 4 | 1,061 | 929 | 1,166 B, 30 keys |
| 5 | 1,274 (too large) | 1,109 | 1,379 B, 36 keys |
| 6 | 1,487 (too large) | 1,289 (too large) | 1,592 B, 42 keys |
| 9 | 2,126 (too large) | 1,829 (too large) | 2,231 B, 60 keys |

T-SIZE measures the real batch sizes; until then the keeper budget (ECON §5.8, O17) shows both the v1 (k = 6) and the v0 fallback (k = 4) costs.

### 6.5 ProgramData reads (C1-15)

The venue ProgramData accounts are 10,485,760 B (pump, pAMM), 948,989 B (pfee), 1,406,429 B (AMM v4) and 793,869 B (CPMM) [V9]; account-data direct mapping is not active on mainnet, so an instruction that lists one copies it into its input region. v0.3 read them in every `open_short` (and designed a per-slot `VenueSlots` mirror as the fallback). v0.4 reads them **only** in `flag_venue_upgraded` and `revalidate_venue`, i.e. once per venue upgrade, so the copy cost no longer matters for opens and the mirror, its per-slot writer and `venue_sync_max_age_slots` are gone (A7; decision 4). The copy's CU cost is still **UNVERIFIED** (U5); `flag_venue_upgraded` requests a generous limit (400 k) until T-CU measures it.

### 6.6 Per-account write-lock budget (C0-17)

Leaders charge each transaction's requested CU against a 24,000,000 CU per-writable-account block limit (`MAX_WRITABLE_ACCOUNT_UNITS`, an unconditional constant in Agave master [V10]; the limits applied by other validator clients are **UNVERIFIED**). Every trading instruction, `crank_price` and `sol_settle` write-lock `Market`, so about 17 transactions requesting 1.4 M CU could saturate a market for a block. Mitigations:

- `claim_sol`, `cover_deficit`, `sync_lender` and `add_margin` do not write-lock `Market` (lender SOL counters, the pot and the lock state live on `MarketVault`, A9). **This isolates lender claims from `Market` only** (V1-6): every exit and `open_short` also write-lock `MarketVault`, so claims still compete with liquidations for that account's 24 M CU per block, and the permissionless `crank_price` write-locks `Market`. A claim needs ≈ 8–20 k CU (§6.1; v0.4 said 25–50 k here, R4A-10 c), so saturating `MarketVault` takes ≈ 1,200–3,000 claims in one block at the keeper's priority price; the effective mitigation is the priority policy below and T-CASCADE, not the account split. A dedicated claims account that exits never touch was considered and not adopted: it would need a second SOL float and a periodic sweep between the two, for a contention that costs an attacker ≈ the whole block budget per slot.
- Keepers request tight CU limits (simulated × 1.1), escalate priority fees under contention, and submit liquidations in Jito bundles (§7.1).
- A market-censoring attacker must fill the 24 M CU budget every slot: ≈ 480× the keeper's per-slot crank cost at the same priority price (ECON §12.6).
- M7 benchmarks cascade throughput (many liquidations on one market in consecutive slots) against this limit.
- A batch of cranks write-locks every `Market` it names; the keeper builds batches from markets with no pending exit in the same slot and simulates each batch (a failing crank fails the batch; `crank_price` fails only on wrong keys, A63).

---

## 7. Off-chain components

### 7.1 Keeper (`services/keeper`, Rust)

- **Stack:** tokio; `services/common` (RPC + Yellowstone gRPC clients); `declare_program!(shortwin)` through `anchor-client 1.2`; **the same** `shortwin-math` and `shortwin-venue-state` crates as the program, so CR, bounds, legs, box and arming decisions match on-chain exactly [R-TOOL §4.3].
- **Inputs:** Yellowstone account subscriptions for all `Market` and `ShortPosition` accounts, each market's venue accounts (curve, pools, vaults, fee configs, `Global`/`GlobalConfig`, AMM v4 `AmmInfo`, CPMM `PoolState`/`AmmConfig`, LP mints, boost vault), **the six venue ProgramData accounts** (the upgrade latch), and every Raydium SOL pool of each listed mint (`getProgramAccounts` memcmp on AMM v4 offsets 400/432 and CPMM offsets 168/200 [R-RAY §2.1, §6.1]); RPC websocket fallback.

**Duties** (ECON §5.8 is normative; the manipulation resistance of ECON §12's atomic model assumes them, A69, R3B-5):

1. **Observation coverage.** At least one observation **per second** on every market with a position whose live CR is below 1.3·L, whose deadline is within 60 s, or that is armed, and wherever `|P_spot/P_ema − 1| > 2 %`; otherwise at least every 30 s for markets with open positions.
2. **Deviation bursts.** After any observation it did not make that deviates from its own view by more than 2 %, observe that market **every slot for 60 s**: atomic bundles then have no second of their own, and the settlement box's deferral residual gets no consecutive attacker-only seconds (ECON §5.6, §12.11).
3. **Liquidate** at the first executable second (fixed `δ_liq`) via a Jito bundle with tip, with a priority-fee RPC fallback; retry each second on `ExitBoundNotMet`; `expire` when the expiry bound is met. Always `simulateTransaction` first.
4. **`sol_settle` every second for armed positions** (and for positions whose S1/S2 legs hold): it arms, keeps the clock running, disarms only on box-robust evidence, or settles.
5. **No-route markets** every ≤ 30 s (continuous S3 chain) and `sol_settle(S3)` when due. While an S3 on an **executable** venue (flag or fee above the hard cap) is due, observe that market **once per second, not per slot** (duty 2's burst does not apply there): the v0.4.1 box takes the medians of bucket lows and highs, so several honest observations at different prices within a second only widen it (§4.5). **Raydium fast path**: once `pool_fail` has held for `pool_fast_route_s`, propose the deepest eligible candidate and call `execute_pool_switch` when its 60 s window closes (§2.6).
6. **Graduation and wipes.** Call pump `migrate_v2` top-level as soon as a curve completes (ADOPTED Q16); on the canonical pool's appearance: `crank_price` (transition, ring rewrite), create `trader_base_ata` if missing, pAMM `init_user_volume_accumulator(payer = keeper, user = trader)`, `extend_account` if the pool is < 300 B. Straddle bundles with `complete_curve` (§5.4) and the "tx1 + tx2 only" fallback. **Right after a full wipe**, `claim_venue_rewards` and `sweep_trader`, so pre-wipe cashback reaches the wiped epoch's lenders (ECON §9.5); since v0.4.1 deposits into a live wipe record also wait for these two (R4A-7), so the keeper's call is a convenience, not the only protection.

**Operations.**

7. **Batching and counting** (R3B-5). A second in which the keeper sends `liquidate`, `expire`, `sol_settle` or `close_short` for a market **is** that market's observation: no separate crank. Remaining cranks of the same second are batched, several markets per transaction, which amortises the 5,000-lamport signature fee (ECON §5.8: 0.43 SOL per market-day unbatched, 0.07–0.10 batched, at per-second coverage of every market). **Batches are v1 transactions** (§6.4; R4B-9): `crank_price` keeps 6 per-market keys inline on PumpSwap / CPMM (5 on the curve), so a v0 transaction with the protocol ALT fits only 4 (5) cranks in 1,232 B; a v1 transaction (4,096 B, 64 inline addresses) fits ≈ 6 shared keys + 6 per market, up to 9 cranks; the keeper uses 6 and falls back to v0 batches of 4 (≈ +50 % base fees) where v1 is not accepted (**UNVERIFIED** until T-SIZE). Batches are simulated; a market whose crank fails simulation is dropped from the batch and cranked alone.
8. **Budget (owner item O17).** Coverage is targeted by duty 1, which costs 0.003–0.03 SOL per market-day at normal priority prices; armed positions, deviation bursts and liquidations cost fractions of a millisol each (ECON §5.8). The owner sets the per-market daily budget, the priority-price ceiling and the markets covered; the keeper exports per-market coverage seconds, CU and lamports spent.
9. **Degraded mode** (A69; ECON §5.8). When the keeper cannot meet duties 1–2 on a market (budget exhausted, outage, contested blocks), throttling must be visible, not silent: without an honest observation in a second an attacker's atomic bundle achieves what ECON's held model assumes, so the §12.6 held-model numbers become that market's exposure.
   - **Opens pause** on that market with reason `KeeperCoverage`: by default the keeper pages the Emergency multisig, which calls `set_pause(market)`; if the owner approves O17's narrow role, the keeper calls `set_coverage_pause(true)` itself (pause-opens-only key, `coverage_authority`). The pause lifts after coverage has been restored for 10 minutes.
   - **On-chain signal**: the program emits `CoverageGap{gap_s, open_positions}` at every integration that follows more than `COVERAGE_GAP_S` (30 s) without one on a market with open positions; the indexer and UI show coverage per market.
   - Existing positions keep their locked rules (A26); their exposure is disclosed as ECON §12.6's held-model bounds. Raising persistence in degraded mode is **not** adopted: persistence is locked per position, and 5 updates already break Tier C liveness (ECON §5.8).
10. **Venue drift monitor and the upgrade latch.** On any change of a venue ProgramData account (pump, pAMM, pfee, AMM v4, CPMM, Jupiter; plus Token-2022 for alerting): call `flag_venue_upgraded(venue)` in the next slot at a high priority price, alert, run the fork suite against the new binary, and page the Emergency multisig to `revalidate_venue`, publish an extras template set or set the route-broken flag as needed (the last two once O6 is signed). Also watch Raydium admin instructions (`SetParams`, `update_pool_status`, `update_amm_config`) and PumpSwap `disable_flags`. Always select the extras set (`none` / `current` / `previous`) that simulates successfully.
11. `crank_sol_usd` every ≤ 30 s with retries (55 s heartbeat; C1-12).
12. **Raydium pool monitor**: track `D` of every eligible pool; on markets with no open position, `propose_pool` a candidate with ≥ 25 % more locked depth (§2.6 rule 2) — in the same slot in which `open_positions` reaches 0 when a much deeper candidate is waiting. If a market stays pinned to a much thinner pool by a standing minimum position (R4B-10), alert and page Emergency to pause opens there so it drains.
13. **Lender pot**: `sync_lender` for lenders holding a surplus when some lender's claim is short of its entitlement (the frontend also prepends it; §3.3). `sweep_protocol_fees` daily; `claim_venue_rewards` weekly for cashback-coin markets; `sweep_trader` when an `ATA(trader, WSOL)` appears.
14. **CU and priority policy** (C0-17): `SetComputeUnitLimit` = simulated × 1.1; priority fee from recent per-account fees on `Market`, escalated when a transaction misses a slot, capped at the O17 ceiling.

- **Keys and ops:** a hot fee-payer key with a small SOL float and no protocol authority (the optional `coverage_authority` is a separate key that can only pause opens). Two regions run concurrently; duplicate liquidations fail harmlessly. Prometheus metrics; alerts on missed liquidations (CR below L for more than N seconds), armed positions not re-checked, no-route markets not cranked, coverage SLO misses and budget burn.

### 7.2 Indexer and API (`services/indexer`, Rust)

- **Ingest:** Yellowstone transaction stream filtered on the short.win program id (and the `complete-curve` helper), plus `getSignaturesForAddress` / `getTransaction` backfill; `emit_cpi!` events decoded from inner instructions and `CoverageGap` from program logs. Also every trade of every listed mint on its venue, for charts and **gap-frequency calibration** (ECON §11.6, G37): pump and PumpSwap events; AMM v4 `ray_log` (base64 bincode `SwapBaseInLog`/`SwapBaseOutLog`); CPMM `SwapEvent` (discriminator `40c6cde8260871e2`, **filtered by emitting program**, because Raydium CLMM and other programs emit an event with the same name and discriminator) [R-RAY §3.1–3.2].
- **Store:** Postgres via sqlx 0.9: `markets`, `market_snapshots` (from `PriceCranked`), `coverage` (gaps, pauses), `pools` (candidates, `D`, λ, switches), `positions`, `position_events`, `lender_positions` (incl. `lock_until`, pot draws), `lender_events`, `share_epochs`, `settlements` (incl. box corners), `fees`, `venue_events` (upgrades, latch flags, admin actions, route changes), `oracle`, `venue_trades`.
- **API:** axum 0.8 REST with OpenAPI plus WebSocket push: `/markets`, `/markets/{mint}`, `/markets/{mint}/pools`, `/markets/{mint}/positions`, `/markets/{mint}/coverage`, `/positions/{pubkey}`, `/lenders/{owner}`, `/stats`, `/events` (WS). Search by mint and ticker, always keyed by mint [R-MINT §10.11]. Analytics per SPEC §8/§13: TVL, utilisation, OI, fees, liquidations, S1/S2/S3 counts, **realised lender loss**, creator-wallet deposits (SPEC R1), **creator-fee income from short.win volume** (C0-18) and keeper coverage.
- The indexer runs the same byte-level mint filter and pool-eligibility checks to show reasons before "Create market" [R-MINT §10.10].

### 7.3 Frontend (`apps/web`)

- **Stack:** Vite + React 19 SPA; TanStack Router/Query; `@solana/kit` 8.3, `@solana/react`, `@solana/kit-plugin-wallet`; the Codama client `@shortwin/client` (zero-copy decoders generated from the IDL's `PodU128` fields, so offsets match the program, A67); `@shortwin/math-wasm` for quotes, CR, liquidation price and bounds. No web3.js v1 [R-TOOL §3].
- **Flows:** market search; create market (filter verdict, venue detection, Raydium candidate pools with `D`, λ, fee and eligibility, cost); lend, withdraw (pro-rata allowance, lock and `lock_until` shown), claim (vesting balance and pot shown; `sync_lender` prepended when the pot must be refilled), cover deficit; open short (Q, slippage → `min_proceeds`, D4 margin preview → `max_margin`, CR at spot and EMA, liquidation price, fees, rent estimate, max duration); position dashboard (SPEC §8 fields plus arming state and box width); `add_margin`; market page (tier and why — including "age counted from listing" under O13 —, adopted pool and fast-path state, inventory, utilisation, rate, OI, caps, keeper coverage, recent liquidations).
- **Transactions:** v0 with the protocol ALT; simulate before signing and show the effects (SPEC §9); detect CPI Guard [R-MINT §8.4]; randomise pump fee recipients from the on-chain lists; prepend idempotent ATA creation and `extend_account` when needed.
- **Disclosures:** SPEC §5.3's exact sentence with a required checkbox; ECON §11.6 break-even numbers; the ECON §9.3 run residual as a function of utilisation (up to ≈ 11 % of a runner's fair value at 50–60 % utilisation); lender lumps vest over 6 h and locks last until the positions they stand for have expired (+1 day); SOL that arrives while a market has no lender goes to the protocol; **the no-route freeze** (V2-5); the borrower-veto residual (ECON §12.9); the fixed 2 % liquidation bound (a keeper can take up to ≈ 2 % of the reference cost from a liquidated borrower, ECON §12.2) and the held-model residual (ECON §12.6); small positions depend on the in-house keeper, and a liquidation keeper is paid at least `K_s` from the borrower's remainder; a market whose keeper coverage lapses pauses opens; venue trust assumptions per market (pump upgrade key; **Raydium: no-timelock 3-of-4 upgrade multisig, unverified builds, 2-of-3 pool admin that can halt swaps or raise fees**); **R1: the coin creator earns the venue creator fee on every short.win leg** (C0-18 (2); ECON §12.8); geo-restriction at the CDN edge per legal advice (SPEC R6).
- **v1.1:** Jupiter quote proxy (the backend holds the API key) [R-JUP §8.6].

---

## 8. Testing and security strategy

### 8.1 `shortwin-math` (host `cargo test`)

- **Unit tests plus proptest** (2,000 cases in CI): round trips never create value; `B̂` monotone in q for every venue family; `reprice` gives `Y/X ≥ P` (R3B-7 c); kinked-rate continuity at the kink; basis-index rent equals the direct integral; `receivable` returns to 0 with no open position under random `rent_protocol_bps` changes (I-P7); fee and penalty splits sum exactly; waterfall legs non-negative and sum to `V` including the keeper top-up (I-E4, I-E10); EMA step bounds and "own observation never read" (I-E7); directional aggregation (+0.00 % with one honest observation); the cached `(x, y)` of every ring entry equals `reprice(l, p)` after every integration and every `rewrite_liquidity`; the EMA leg reads only the newest `liq_persist_n` entries; **settlement box**: every predicate evaluated on the box equals its value at the named corner, one pushed second cannot change a box decision, and in a sustained uptrend the S1 corner is `P_ema` (I-E5, I-E8; §4.5); exit-bound properties with the fixed δ, the `lo_x` cap, no reserve in the liquidation bound and rent and the expiry fee senior in the expiry bound (v0.4.1; I-E1–I-E3, I-E11); S1 take = `V − K_s` and S1 whenever its legs hold on the box (I-E5); `D = λ·√(X·Y)` changes under swaps only by the retained fee (V2-7) and not under unlocked LP; lender pool: pot draws never exceed the own deficit, repeated claims pay 0, liveness after a full wind-down (I-P6), no lock effective at or after `lock_until`, vest release linear and complete at `vest_end`, credits with `S = 0` to the wipe record or the protocol; cap closed forms against brute force; U256 against num-bigint; `params::validate` equals `shortwin_sim.validate_params` on random configs, every compiled edge and the config gate's configurations, and `params::validate_econ` bounds exactly the parameters the sim does not model (T-CONFIG).
- **Differential tests against mainnet** (fixtures in `tests/fixtures/txs/`, every amount to the raw unit):
  - curve: `5hRKzXWt…`, `3pu4FB7r…`, `3QHQnT3S…`, `38PUmRjY…`, `cHHjJZFF…`, `535njpqh…` [R-PUMP §4.1];
  - PumpSwap: TX-BUY `udGBnNtp…`, TX-BUYQ `41XkmCLx…`, TX-SELL `2oSvEbDT…` [R-VEN §2.4];
  - Raydium (T-RAY-MATH): every signature in [R-RAY §0, §3.5, §4.1] — TX-V4-OUT2 `27EekaMR…`, TX-V4-OUT2-TOP `4PvobD1K…`, TX-V4-IN2 `4s6yJp8B…`, TX-V4-OUT1 `5Y8sTSnT…`, TX-V4-IN1 `5mXDiWRq…`, TX-CP-OUT `4dijJfUL…`, TX-CP-OUT-TOP `dR9wZQZw…`, TX-CP-IN-TOP `3Pp4LzfN…`, TX-CP-CFIN `qzC7eoYk…`, TX-CP-CFOUT `5vjTcH3h…` — plus the research's program-wide samples (388 AMM v4, 285 CPMM swaps);
  - ECON T1–T11.
- **Differential tests against ECON's models — v0.4.1, no exceptions** (R3A-3): port `sim/pool_model.py` **v0.4.1**'s fuzz (60,000 ops per configuration: continuous rent at 90 % and 100 % utilisation caps, repeated wipes with and without the post-wipe sweep, covers, governance changes of `rent_protocol_bps`, claims ×N, churn, sweeps) and its lock-heavy `lock_fuzz`, and assert **identical state traces** after every operation (lock calendar included), including the untouched-state I-P4 and the wind-down liveness check every 2,000 operations (I-P1–I-P7); compare `sim/shortwin_sim.py` **v0.4.1** quotes (incl. `B̂_str`; `B̂₂` only as a v1.1 unit test), observation/EMA/ring traces (incl. the cached `(x, y)` and venue-change rewrites), exit bounds, settlement-box decisions and waterfalls on the deterministic scenarios of ECON §11.2, the fast-path scenarios of ECON §10.3 and the 1,000-path exit fuzz of ECON §13 (S2 and S3 exits, noisy and freezer runs). Rent is compared **exactly**: the sims use the program's three basis indices, so the v0.3 "≤ 1 lamport per accrual" tolerance is gone; the S2/S3 lump is computed from `⌊Q·P_ema/WAD⌋ + R_due` in both (v0.4.1, R4A-5). The attack scripts of `sim/attacks/` provide the named traces (`fixed_delta.py`, `refute_push.py` with push-then-freeze and widening, `fv_push.py` with the S2 → S1 flip, push-then-freeze, consecutive-push and `p'` veto cases, `ptf_sandwich.py`, `venue_switch.py`, `pool_r3.py`, `cascade.py`); the config gate's 1,655-run battery is T-CONFIG's host part.
- **CU budgets** (T-CU-MATH, M0b): a Mollusk bench per primitive of §6.1 fails CI above the measured value + 20 %.

### 8.2 Program tests (LiteSVM 0.16, Rust)

- **Load:** `target/deploy/shortwin.so` and `complete_curve.so`; **dumped mainnet ELFs** recorded in `fixtures/manifest.json` with sha256: pump (deploy slot 449,734,335), pAMM (446,462,733), pfee (446,465,969), Raydium AMM v4 (445,763,204; trimmed ELF sha256 `d4cee0cd…2c9f`), Raydium CPMM (445,763,504; `bdaff73e…f8cc`) [V9; R-RAY §1.3]; the test-only `mock_cpamm` for M2. Never use the `pump.so` bundled in `pump-rust-client` and never calibrate on devnet [R-PUMP §9.11].
- **Account fixtures:** pump `Global`, both `FeeConfig`s, pAMM `GlobalConfig`, a live curve, a canonical pool with non-zero VQR, a Raydium-graduated curve (`TBHe5tJn…` [V3]); Raydium: Fartcoin AMM v4 pool `Bzc9NZfM…` with vaults and LP mint, the tiny reversed-order Fartcoin pool `9GWMrDgH…`, USELESS CPMM `Q2sPHPdU…` with config and observation, creator-fee pool `FQd8z3…`, a Token-2022 CPMM pool `7ZRtFcgf…`, all 21 `AmmConfig`s; mints; the six ProgramData accounts (for the latch).
- **Scenario builders:** `set_account` rewrites curves and pools (gaps 2×/5×/10×, drains, completion, graduation, disable flags; AMM v4 `status`/`pool_open_time`/`swap_fee`/`need_take_pnl`; CPMM `status`/`open_time`/fee counters/`AmmConfig` rates; LP burns and unlocked deposits; ProgramData deploy slots); `set_sysvar::<Clock>` / `warp_to_slot` drive time.
- **Invariants after every instruction** (§8.3).
- **Test gates** (must pass before the dependent milestone closes; ★ = new in v0.4):

| Gate | Question it settles | Milestone |
|---|---|---|
| ★T-LAYOUT | Every zero-copy type has the same `size_of` and field offsets on sbpfv3, x86_64 and wasm32 (compile-time asserts on all three builds), contains no raw `u128`/`i128`, and the Codama decoders read the same offsets | M0b / M1 |
| ★T-CU-MATH | Per-primitive CU within the §6.1 measured value + 20 % | M0b |
| ★T-CONFIG | `params::validate` equals `shortwin_sim.validate_params` (random configs, every compiled bound at its edges incl. the per-tier bounds and the split's parts, the per-tier SPEC CR minima), `params::validate_econ` bounds exactly the parameters the sim does not model, and the program's `initialize_config` / `propose_config` refuse exactly what the two refuse; `scripts/config-gate.sh` rejects a proposal with a liveness row in `config_check.py`'s 1,655-run battery (v0.4.1: noise, grid, aged; sizing parameters compared at equal position size) and requires an owner sign-off for lender-income-only rows | M1 |
| ★T-LATCH | `flag_venue_upgraded` sets the bit only on a changed deploy slot and refreshes `flagged_slot` on a second change while set; opens refused until `revalidate_venue(venue, expected_slot)`, which fails on any other slot (a second upgrade during review); a pAMM upgrade blocks curve opens and a CPMM upgrade blocks AMM v4 opens (masks, R4B-7); exits unaffected; no write when unchanged | M1 / M3 |
| ★T-POT | Lender wind-down: stranded SOL ≤ the dust bound (pool_model's liveness test and the reviewer's 40-seed stranding table); JIT lump capture 0 at `h = 0`; stuck-lock scenario clears at `lock_until`; v0.4.1: **an idle lender past its `lock_until` while the others withdraw first** gets its pro-rata part (the reviewers' `stale_lock.py`), and I-P4 holds on the untouched state over the lock-heavy mix; a lock outlives a position stuck in `PumpMigrating` with compiled-maximum timers; 1-lamport sweeps every minute leave ≥ 98 % of a lump released after 6 h; a deposit into a live wipe record fails until both post-wipe sweeps ran; a whale around an S2/S3 of the last position captures 0 | M1 |
| ★T-BOX | Settlement box on the mock venue: S2 → S1 flip with 1–2 pushed seconds fails, **and with 1 push + 0–10 frozen seconds** (v0.4.1, R4B-2), including the Raydium `L` variant (an LP remove in the push second, LP add/remove around the keeper's observation in the frozen ones); 5 consecutive attacker-only seconds are the documented limit (R4A-9); a borrower's refutation push, or push-then-freeze, before maturity delays S1 by ≤ 8 s; spot-leg vetoes keep the clock; amount push ≤ `fv_dev_bps`; **a contrary low observation before `sol_settle(S3)` does not change `p'`** (R4A-3); a halted venue's S3 is not blocked (box collapses after `RING_LEN` s); proptest "one pushed or frozen second cannot change a box decision" | M2 |
| ★T-VENUE-SWITCH | After graduation and after a Raydium fast path, every ring entry's `l`, `llo`, `lhi`, `(x, y)` equal the reset liquidity and the box is ≤ 1 % wide in `L` from the first second (ECON §10.1) | M4 / M6 |
| ★T-COVERAGE | `CoverageGap` emitted exactly when an integration follows > 30 s with open positions; `set_coverage_pause` inert while the authority is unset, pause-opens-only when set | M7 |
| T-RAY-MATH | §4.3 Raydium formulas equal the dumped ELFs on random states and every listed mainnet trade, incl. CPMM creator fee on the output side | M3 |
| T-RAY-PDA | Trader PDA as owner/payer via `invoke_signed`; `pool_vault` as delegated source; `pool_vault` as buy destination; delegate cleared at 0 | M3 |
| T-RAY-DISABLE | Route predicate vs the ELFs: AMM v4 status 2–5, 7 with future open time, 8; CPMM bit 2 and future `open_time` | M3 |
| T-RAY-FEE | Config/pool fee changes with positions open: opens refused above `venue_fee_cap_bps`, `s3_no_route` and `pool_fail` above the global hard cap, FV fee ceiling at the locked cap | M3 |
| T-RAY-WSOL | Pre-funded `trader_wsol` address, WSOL donations, a pre-created `ATA(trader, WSOL)`: all harmless | M3 |
| T-RAY-TRACE / T-RAY-CU | §6.1 / §6.2 figures for Raydium instructions | M3 |
| T-RAY-SWITCH | Fake deep pool, LP pull, proposal spam, atomic price push at proposal, adoption refused with open positions, fast path on `pool_fail` (never on the flag), listing window | M4 |
| T-PDA-USER | Pump `buy_v2` and PumpSwap `buy` with the trader PDA as `user` (System debits from `user`, venue-created accounts). Sells are already on mainnet (C1-7) [R-PUMP §6.1; R-VEN §3.1] | M5 / M6 |
| T-RENT | Venue-side rent on buys **and sells** stays ≤ `exec_rent_tol` and is borne as designed (C1-4, C1-9) | M5 / M6 |
| T-ATA | Does PumpSwap accept the non-ATA `trader_wsol` owned by `user`? Do pump/pAMM require the base ATA? [V11] | M6 |
| T-DISABLE | Which `disable_flags` bit disables buy/sell. **Hard gate**: the constant is set from it (C0-10) | M6 |
| T-STRADDLE | The ≥ 3-transaction bundle of §5.4 with `complete_curve`: CU, size, trace per transaction; ★a curve trade landing between the keeper's read and the bundle; ★curve already completed by a third party (tx1 no-op) | M5 / M6 |
| T-CASHBACK | `claim_cashback` (pump and pAMM) to the trader without its signature; proceeds vest to lenders | M6 |
| T-REPLAY-0428 | Pump's 2026-04-28 account change replayed against exits, recovered with an extras set (after O6) | M6 |
| T-REPLAY-PDA | A synthetic venue change that adds a **per-mint PDA** remaining account is recovered with a derivation template; a hostile `current` set is bypassed with `previous` or `none` (V2-6) | M2 (mock) / M6 |
| T-S3-BROKEN | A flagged route (and a venue whose CPI fails, and a venue-state account whose layout changes) reaches S3 after `s3_wait_s`; **with the flag set, the liquidatable set, every close outcome and the `pool_fail` chain equal the unflagged run** (V2-1, V1-7, V1-8, R3A-4) | M2 / M6 |
| T-TRACE | Instruction-trace counts per instruction and per bundle transaction ≤ §6.2 | M2–M6 |
| T-SIZE | Worst-case transactions ≤ 1,232 B with the protocol ALT; ≤ 64 accounts; the number of cranks per batch | M3–M7 |
| T-CU | Whole-instruction CU ≤ §6.1 limits under the mainnet feature set (p-token), incl. `flag_venue_upgraded` with a 10 MiB ProgramData | M3–M7 |
| T-DEPTH | Every venue path through one wrapper level; a missing fee ATA fails cleanly (C1-8) | M5 / M6 |
| T-MEMO | A CPI'd memo satisfies the MemoTransfer sibling check [R-MINT §11] | M10 |
| T-CASCADE | Many liquidations on one market in consecutive slots under the 24 M CU per-account limit (C0-17); the reviewer's `cascade.py` sequence | M7 |

- **Adversarial suite** (C0-12; each runs on the mock venue in M2 and on every real venue as it lands):
  - wrong, forged or **omitted** venue accounts on every instruction (C0-2, C0-15); non-canonical PumpSwap pool; spoofed curve owner; wrong creator vault; sensitive accounts as fee recipients or extras; a thin Raydium pool in place of the adopted one; a wrong ProgramData for `flag_venue_upgraded`;
  - trader griefing: lamport donations to the trader and to the `trader_wsol` address, token donations to `trader_base_ata`, a pre-created `ATA(trader, WSOL)` with WSOL (C0-3, C1-3); dust donation to the boost-vault ATA (C1-2);
  - **sandwich around `liquidate` and `expire`** at the fixed bound, acting at once and after waiting (ECON §12.2 `fixed_delta.py`); **held-model sandwich** (the penalty reserve keeps lenders whole, ECON §12.6); **atomic `[pump, crank, dump]` every second**, near-threshold pushes, fresh-position pushes, OI-basket forcing (ECON §12 scripts ported);
  - **round 3** (ECON §12.10–§12.11): S2 → S1 flip with 1–2 pushed seconds plus `[pump → sol_settle → dump]` (must fail; R3A-1); a borrower's one-second refutation push before maturity, repeated each cycle (S1 ≤ 4 s later; R3A-6 a); spot-leg vetoes of 12–20 s (clock keeps running; R3A-6 b); amount push up and down (≤ `fv_dev_bps`); deferral by repeated attacker-only seconds (keeper duty 2 bounds it); Emergency-flag S3 raced against an honest expiry (O15);
  - second round: block the spot leg with one low observation per second (it can no longer widen the bound); the borrower's defensive veto and "contrary observation every second" (V1-2, V2-14); atomic `[dump → sol_settle → buy back]` and `[deposit LP → sol_settle → withdraw LP]` every 20 s for 10 min (V2-4); unlocked LP added to the adopted pool (V2-8); fast-path front-running with an own pool and wash trading to raise `D` (V2-7); a venue that over-delivers tokens on a buy (V2-10); forced EMA-parameter adoption with positions open and the kink adopted only at 0 positions (V2-13, R3A-9);
  - lender: JIT deposit around rent and **around lumps** (`[deposit → expire / liquidate / claim_venue_rewards / sweep_trader → withdraw → claim_sol]`: 0 at `h = 0`; R3B-2); first-depositor inflation; **repeated full SOL settlements** (C0-6); **withdraw-then-settle in one transaction** (C0-9); early claim then shortfall and **exit with negative pending** (C0-13); **`claim_sol` ×N in one transaction** (V2-2); **withdraw between accrual and realisation, then wind down** (nothing stranded beyond dust; R3A-2); **open/close churn between a runner's withdrawals** (locks never decrease before `lock_until`; V2-3) and **steady borrowing after a withdrawal** (the lock clears at `lock_until`; R3B-1); **SOL credited after a full wipe** and a dust deposit (V2-12); **all lenders withdraw, then a sweep and a dust deposit** (the SOL goes to the protocol, not the depositor; R3A-10); **`rent_protocol_bps` changed with positions open** (no phantom receivable; R3A-5);
  - config: adversarial in-bound parameter changes with positions open (snapshots hold; C0-8); every compiled corner passes `config_check.py`; a change of the global hard cap moves only S3 / fast-path timing (A26); fee-config changes up to the venue's maximum; Raydium admin disable/fee hikes (the liquidatable set is unchanged above the hard cap; V2-1);
  - venue: ProgramData slot change (the latch refuses opens after `flag_venue_upgraded`; exits work), **venue ABI change with positions open** (C0-4), a venue-state layout change (not executable, S3 reachable; V1-8), completing buyback, straddle (tx3 priced by `B̂_str`, fallback without tx3, trade before the bundle), stuck migration (S3 at 70 min), boost window, creator change mid-position, mayhem and USDC curves at listing, zero-lamport borrower; small positions liquidated with the keeper floor (R3B-10).

### 8.3 Invariants (checked in LiteSVM after every instruction and in Crucible)

| # | Invariant |
|---|---|
| I1 | `T + ΣB_Q` = total lender token claims (SPEC §10); `pool_vault.amount ≥ T` (donations ignored) |
| I2 | `ShortPosition.lamports ≥ rent_min + v`; `MarketVault.lamports ≥ rent_min + lender_sol + protocol_fees` |
| I3 | Position SOL leaves only via close / liquidate / expire / sol_settle (SPEC §10) |
| I4 | No admin-signed instruction decreases `pool_vault`, `ShortPosition` or `MarketVault` balances, except `sweep_protocol_fees` by ≤ `protocol_fees` to `config.fee_recipient`; no instruction changes a venue program id (SPEC §10; C0-8); `coverage_authority` can change only `paused_coverage` |
| I5 | Utilisation ≤ `util_cap` immediately after every open (SPEC §10) |
| I6 | `ΣB_Q` = Σ open positions' q; `rent_aggr_j` = Σ q·coef_j·(10⁴ − rent_protocol_bps_pos); `open_positions` matches; `max_release ≥` every open position's deadline + its snapshot S3 tail (v0.4.1) |
| I7 | After every instruction that ran a leg: trader 0 lamports and System-owned with no data; `trader_base_ata.amount == 0` with owner/delegate/close authority unchanged; `trader_wsol` closed; `pool_vault.delegate == None` (§2.4 rule 5) |
| I8 | = ECON I-P1–I-P7: token claims ≤ assets; **claim side, strict per lender**: `Σ(real − claimed) + unmatched + vest_bal ≤ lender_sol`, `claimed ≤ real`, and after a touch `real − claimed ≤ max(pending, 0)`; entitlement side, signed aggregate: `Σ pending ≤ lender_sol − vest_bal + receivable`; `S + VS ≤ R_MAX·(T + ΣB_Q + VA)` or `S = 0`; withdrawals ≤ pro-rata allowance; a repeated `claim_sol` pays 0; locks never decrease under churn and **no lock is effective at or after its `lock_until`**; **Σ allowances ≤ T + dust on the state the program sees, each lender touched alone** (v0.4.1, R4A-2/R4B-3); the calendar's `lock_norm` equals the sum of its live buckets and of the live locks' terms; no share reduction with `pending < 0`; **liveness** after a wind-down (Crucible runs it on a fork every N actions); `receivable = 0` and `A_j = 0` with no open position (V1-10, V2-2, R3A-2, R3A-5, R3B-1) |
| I9 | Every waterfall conserves `V` exactly (ECON I-E4) |
| I10 | Closes, liquidations, expiries and settlements never read `oracle_cache`, `venue_guard`, pause flags or the route-broken flag as a gate; no quote, leg, bound or `pool_fail` decision reads the flag (structural test on account lists plus a fuzz action that pauses and flags everything; the flag may only *enable* S3 for positions liquidatable — spot leg evaluated normally — or expired; V2-1) |
| I11 | ECON I-E1–I-E3, I-E11: keeper-initiated venue spend ≤ `⌊B̂_ref·(10⁴ + δ)/10⁴⌋` with the position's **fixed** δ and ≤ `max_in`; measured `B ≤ max_in + ert`; an **expiry's** rent and fee paid in full whenever `V − R − π_ref − ert ≥ B̂_ref·(1 + ε)`; penalty/expiry fee ≤ rate × `min(B̂_live, B̂_ref)`; **liveness (v0.4.1): a liquidatable, buyback-allowed position with a fresh bucket never gets `ExitBoundNotMet` while `B̂_live ≤ ⌊B̂_ref·(10⁴ + δ)/10⁴⌋`** |
| I12 | ECON I-E5, I-E8, I-E9, I-E10: S1 take = `V − K_s` and its EMA leg holds at `(P_lo, L_hi)`; S2/S3 take = `min(V − K_s, FV(P_ema, L_ema))`, inside `[x(P_lo, L_hi), x(P_hi, L_lo)]` (±2 lamports), with the corners within `fv_dev_bps` and each corner's `p'` decided at that corner (v0.4.1); S1/S2 only after `settle_persist_n` EMA updates of one clock that only box-robust evidence resets; liquidation only with the EMA leg on the newest `liq_persist_n` entries; S3 only after a continuous chain ≥ `s3_wait_s`; a liquidation that leaves the borrower equity pays the keeper ≥ `K_s` |
| I13 | ECON I-E6, I-E7: venue charge ∈ `[B̂ − 5, B̂]` (quotes are upper bounds; `reprice` gives `Y/X ≥ P`); EMA single-step bounds; no instruction reads its own observation; every ring entry's `(x, y) = reprice(l, p)` |
| I14 | Every account that feeds pricing, route, tier or guard is present and key-checked (structural test over all instruction account lists; C0-2) |
| I15 | A Raydium market's adopted pool satisfied `eligible()` when adopted; a non-fast adoption happened only with `open_positions == 0`; every open was sized on liquidity ≤ locked depth / `λ_min` and satisfied the `R_eff` and fee checks at that moment (V2-8, decision 7) |
| I16 | Every risk quote (CR legs, S1/S2 legs, box corners, refutation, `B̂_ref`, FV) uses `min(φ_live, venue_fee_hard_cap_bps_pos)`; no observation is recorded while the venue is not executable (except `PumpMigrating`); `pool_fail` never depends on the route-broken flag (V2-1, V2-5, R3A-4) |
| I17 ★ | No open is accepted while `venue_guard.upgraded` has a bit of the market's venue mask set (A7; masks v0.4.1) |

### 8.4 Fuzzing, static analysis, fork E2E

- **Crucible** via `anchor fuzz`, nightly and stateful, invariants I1–I17. Actions: all instructions, plus price shock, liquidity drain and add (locked and unlocked), complete curve, migrate, disable pool (PumpSwap flag, AMM v4 status, CPMM bit), fee hikes, warp clock, **venue ProgramData slot change** (with and without `flag_venue_upgraded`), **atomic `[pump, crank, dump]` bundle every second**, **contrary (low) observation every second**, **atomic dump or LP deposit around `sol_settle`**, **attacker-only seconds before a settlement** (push up, push down, flip attempt; **1–6 consecutive**), **push-then-freeze** (one push, then an opposite observation in every second that also holds the keeper's), **contrary low observation before `sol_settle(S3)`**, **a contrary observation every second while S1 or S3 is due** (widening; deferral only), **sandwiched exits** (atomic and held), **omit/replace a remaining account**, **venue-state layout change**, **donate to trader / `trader_wsol` address / `ATA(trader, WSOL)` / boost ATA**, **venue CPI failure + route-broken flag**, **extras set switch**, **repeated full settlements**, **withdraw-then-settle**, **`claim_sol` ×N**, **churn between withdrawals**, **steady borrowing past `lock_until`**, **an idle lender past `lock_until` while others withdraw**, **JIT deposit around a lump and around an S2/S3 settlement**, **1-lamport sweeps every minute**, **a dust holder that stays while the other lenders exit**, **all lenders out, then a sweep**, **deposit after a full wipe with and without the post-wipe sweeps**, **adversarial in-bound config change** (incl. `rent_protocol_bps`, the global hard cap), **forced EMA adoption**, **keeper silence for > 30 s** (CoverageGap), pool proposals with and without open positions, fast-path windows and switches [R-TOOL §2.2; C0-12].
- **Static:** clippy with the denies of §2.2; a lint that rejects raw `u128`/`i128` in `state/` (A67); cargo-deny and cargo-audit; Radar; IDL and Codama diff checks; Token-2022 extension CI guard (dev-dependency `spl-token-2022-interface =3.1.2` fails on any unclassified type) [R-MINT §9.5]; a CI check that the release artifact is built without the `test-venue` feature (A49); the T-LAYOUT asserts compiled for all three targets.
- **Fork E2E:** Surfpool 1.6 with its Pump/PumpSwap templates (fidelity after the 2026-09-23 pump deploy **UNVERIFIED**) and cloned Raydium pools (Fartcoin AMM v4, USELESS CPMM). Keeper, indexer and web (Playwright) run end-to-end; `surfnet_timeTravel` covers expiry, arming windows, vesting, lock expiry and S3.

### 8.5 Security posture and trust assumptions (disclose)

- **Upgradeable dependencies.** pump, pAMM and pfee share upgrade authority `7gZuf…` and ship several upgrades a month [R-PUMP §1; R-VEN §7]. Raydium AMM v4 and CPMM share a Squads v4 upgrade multisig (3 of 4, **no timelock**) and have **no verified builds**; the pool admin (Squads v3, 2 of 3, no timelock) can halt individual pools and raise fees [R-RAY §1, §5]. Token-2022 is upgradeable by `AeLmXC…` [R-MINT §2]. Our own program (and the `complete-curve` helper) is upgradeable by the Governance multisig (48 h).
  - The venue guard pauses **opens** only, and only once the latch is set: an open that lands between an upgrade taking effect and `flag_venue_upgraded` (the keeper's reaction time, normally the next slot) is accepted. A malicious venue upgrade can still harm exits; no design can prevent that (SPEC R5). The route-broken flag (after O6) and S3 bound the time **liquidatable** positions stay frozen; healthy positions on a halted or broken venue can be neither closed nor settled until the route returns, an upgrade adapts the adapter (48 h), or their deadline (up to 30 days); owner options in §10 O14 (V2-5).
- **Admin powers of short.win** (§3.1): timelocked parameters inside compiled bounds, each proposal gated by the ECON liveness battery; Emergency: pause opens, disable a venue for opens, re-validate a venue, and (after O6) the route-broken flag and bounded extras; the optional `coverage_authority` pauses opens only. No fund movement; no venue-id changes.
- **Creator self-dealing (R1):** the coin creator earns venue creator fees on every short.win leg (§7.3; ECON §12.8).
- **Residual economic risks** (ECON §8.2, §9.4, §12.2, §12.6, §12.9–§12.11): the fixed 2 % bound lets a keeper take up to ≈ 2 % of the reference cost from a liquidated borrower at the first second (Tier A +1.50 SOL over an honest keeper, borrower −2.02 SOL; v0.3 +0.71 at once but +3.86 after 4 s); under the held model a keeper takes the remainder of a borrower already being liquidated (Tier A +5.66 SOL, borrower −19.5 SOL) and, because a liquidation reserves nothing (v0.4.1), lenders and protocol lose the penalty and — on positions whose rent due exceeds `K_s` — the rent that the bought-down vault no longer covers (Tier A −2.06 SOL fresh, −8.02 SOL aged 25 days at 90 %; v0.4's claim that "lenders are never worse off thanks to the penalty reserve" was wrong: that reserve blocked honest liquidations and did not hold on aged positions); low-fee Raydium baskets stay marginally profitable at the OI cap (+0.5–8.5 SOL); the borrower's paid veto of its own liquidation; a settlement-amount push of at most `fv_dev_bps` (±1 %), a settlement deferral of ≈ 13 s per attacker-only second, and (v0.4.1) an indefinite, paid deferral of S1 or an S3 on an executable venue by a contrary observation every second (0.002–0.21 SOL/s); **5 consecutive attacker-only seconds** (degraded mode, a censored keeper) move the box's medians and flip a capped S2 into S1 on the drained-RA model cases (+2.37 to +5.58 SOL for 1.1–2.0 SOL); a lender that exits forfeits its unreleased vest to whoever stays; a standing minimum position can pin a Raydium market to a thinner pool (§2.6); the ECON §9.3 run residual (up to ≈ 11 % of a runner's fair value, depending on utilisation); rebase-orphan dust (negligible except in dust-sized pools, ECON §9.5); in degraded mode, a market's exposure is the held-model bound. All disclosed; owner item O19 accepts them.
- Before mainnet: external audit (SPEC §9), `security.txt` / `security.json` [R-TOOL §5.1], verifiable build, bug bounty.

---

## 9. Milestones

Each milestone is small, merges behind CI, and has testable exit criteria. **M0, M0b and M1 involve no venue at all; M2 builds the whole position engine against a test-only mock venue.** Real venues then arrive one per milestone, Raydium first because it is the simplest adapter (no venue-side rent, no fee tiers, 3 trace entries per swap, no graduation).

| ID | Title | Scope | Exit criteria |
|---|---|---|---|
| M0 | Scaffold + math crate (**v0.3**; in progress) | Repo layout (§1.2) with pinned toolchains and lockfile (C1-14); CI (fmt, clippy denies, deny/audit); `shortwin-math` against the **v0.3** sims: U256 (Knuth D), both constant-product quote families, `B̂_str` (and `B̂₂` for v1.1 only), observation/EMA/ring, persistence and arming, exit bounds, rent basis indices, CR/legs/FV, caps, tiers, waterfalls, lender pool (epochs, pro-rata, realised claims), locked depth; proptests; differential tests vs `pool_model.py` / `shortwin_sim.py` **v0.3** (R3A-3: v0.3's M0 text said v0.2 and "two-leg quotes"; both were wrong) | `cargo test -p shortwin-math` green incl. differential suites and 2,000-case proptests; WASM build of the math crate |
| **M0b** | **Math crate v0.3 → v0.4.1** | Exactly the delete / change / add list below, against `docs/design/sim/*` **v0.4.1** | Every differential suite green against the v0.4.1 sims **with no tolerance or exception** (exact rent; pool fuzz traces incl. the liveness wind-down; the 1,000-path exit fuzz incl. S2/S3 and box decisions; the attack traces of §8.1); T-CU-MATH; T-CONFIG (host part); WASM build; the crate README's design notes updated (the ones v0.4 settles are closed) |
| M1 | Core program, no venues | `initialize_config`, `propose/apply/cancel_config` (timelock, compiled bounds, relational checks, the config gate script), `set_pause`, `disable_venue`, **`VenueGuard` + `flag_venue_upgraded` + `revalidate_venue`**; `create_market` for any SPL/T22 mint as `None` (detection keys derived and stored; filter); `deposit`, `withdraw` (pro-rata, locks, `lock_until`), `claim_sol` (pot), `cover_deficit`, `sync_lender`; `sweep_protocol_fees`, `flag_unsafe_mint`; `OracleCache` + `crank_sol_usd`; events; Codama client; `PodU128` layouts. `set_coverage_pause` only if O17 approves the role. **`set_route_broken` and `set_venue_extras` are not built here** (O6) | LiteSVM green; I1, I2, I4, I8, I17 after each instruction; T-LAYOUT; T-CONFIG; T-LATCH (mock ProgramData); T-POT; one filter fixture per extension; timelock, bounds and pause tests; lender adversarial cases (C0-6, C0-9, C0-13, R3A-2, R3A-10, R3B-1, R3B-2); CU baseline bench |
| M2 | Position engine on a mock venue | Test-only `mock_cpamm` program (constant product, fee on input, SPL-Token WSOL side, exact in/out, delegate-aware, admin disable and fee knobs, **layout-change and over-delivery knobs**) and `VenueKind::Mock` behind the `test-venue` feature (A49); prologue (params and kink versions, accrue with vest release, roll with the cached reprojection, record only while executable, S3 and `pool_fail` chains, `CoverageGap`); `open_short`, `close_short`, `add_margin`, `liquidate` (fixed δ, no reserve, keeper floor), `expire`, `sol_settle` (**settlement box**, S1/S2/S3, epoch records, vesting lumps); trader normalisation and ephemeral WSOL; extras parser with templates and sets and `set_route_broken` / `set_venue_extras` **only if O6 is recorded**; snapshots; keeper v0 (duties 1–4, batching) against LiteSVM | ECON §12 attack scripts reproduced against the program on the mock with ECON's outcomes (`sandwich`, `fixed_delta`, atomic bundles, near-threshold, basket, `block_spot`, `arming`, `refute_push`, `fv_push` incl. the S2 → S1 flip, `cascade`); I3, I5–I7, I9–I14, I16; T-BOX; T-S3-BROKEN; T-REPLAY-PDA (mock, if O6); T-TRACE for the engine |
| M3 | **Raydium AMM v4 + CPMM** | Readers, fees (ppm) and exact math; `create_market(venue_hint)` with verification and eligibility; delegate sells and direct-to-vault buys; route predicates; fee caps; the latch for both programs; admin-action monitoring in keeper v1; Raydium ALT entries | T-RAY-MATH, T-RAY-PDA, T-RAY-DISABLE, T-RAY-FEE, T-RAY-WSOL, T-RAY-TRACE/CU, T-SIZE and T-CU for Raydium; T-LATCH on the dumped ProgramData; fork E2E on Fartcoin (AMM v4) and USELESS (CPMM) through open → liquidation → S2/S3 |
| M4 | Raydium pool selection | `propose_pool` (listing, zero-position adoption, fast-path window), `execute_pool_switch` (fast path only), `D` metric, `pool_fail` chain, ring rewrite; keeper pool monitor and fast-path duty | T-RAY-SWITCH; T-VENUE-SWITCH (Raydium); I15; a fast path with open positions keeps locked terms and EMA continuity; adoption refused while positions are open |
| M5 | Pump curve + `complete-curve` helper | `classify_pump` and curve detection; curve adapter (`sell_v2`/`buy_v2`), UVA at listing, `ert` funding on sells, `B̂_str` straddle pricing on live and EMA legs (cached `(vT_i, vQ_i)`), completing buyback, migration pause, keeper `migrate_v2`; the `complete-curve` program (A66) | T-PDA-USER (curve buy path), T-RENT (curve), T-DEPTH, T-TRACE, T-CU, T-SIZE; T-STRADDLE (curve part, incl. the trade-before-bundle case); ECON §11.2 curve scenarios; stuck-migration S3 at 70 min |
| M6 | PumpSwap + graduation | `→ PumpSwap` transition with the ring rewrite, pool caches, WSOL per T-ATA, existence requirement, boost gate, `disable_flags` handling, straddle bundle, `claim_venue_rewards`, `sweep_trader`; keeper graduation duties | **T-DISABLE (hard gate, C0-10)**, T-ATA, T-PDA-USER (pAMM buy path), T-RENT (pAMM), T-STRADDLE, T-CASHBACK, T-REPLAY-0428 (if O6), T-VENUE-SWITCH (graduation); Surfpool E2E through a real graduation; ECON §10 graduation scenarios |
| M7 | Tiers, caps, fuzzing, load, keeper economics | USD tiers with the oracle cache; slow-EMA liquidity; fee-scaled and wallet caps; launch guard; Crucible harness with all invariants (nightly); cascade benchmark; **keeper cost measurement** (per-market coverage, batches per transaction, CU per instruction) feeding back into ECON §5.8; degraded-mode drill | Fuzz clean ≥ 24 h; tier-transition and cap-boundary tests; T-CASCADE; T-CU; T-SIZE (batch size); T-COVERAGE; ECON §5.8's cost inputs replaced by measurements |
| M8 | Indexer + API | Ingest incl. Raydium swaps, admin events, latch flags and coverage; Postgres schema; REST/WS; analytics incl. realised lender loss, creator-fee income and keeper coverage | Replays a Surfpool E2E session into correct DB state; API contract tests |
| M9 | Frontend | All §7.3 flows with simulation-before-sign and disclosures | Playwright E2E on Surfpool; CPI Guard and memo UX tests |
| M10 | Hardening + audit | Adversarial suite complete; T-MEMO; verifiable build pipeline (C1-5 syntax) for both programs; Squads Governance/Emergency; security.txt; gap-frequency calibration report from M8 data; external audit | Audit findings resolved; calibration reviewed by the owner |
| M11 | Guarded mainnet beta | Deploy via Squads; `market_max_notional` 25 SOL; in-house keeper in 2 regions with the O17 budget; monitoring | Every §10 decision recorded in SPEC §14; runbook drills: pause, venue upgrade (latch and revalidate), route-broken (if O6), stuck migration, Raydium admin disable and fee hike, keeper outage (degraded mode) |
| v1.1 | Extensions | Jupiter adapter (D1); delegate-based deposit for CPI Guard; in-program straddle (`ExitInProgress`); partial close; CPMM TWAP bound; withdrawal queue (O10); concentrated-liquidity pricing (D2); borrower-initiated SOL close during a long no-route period (O14 b, a SPEC amendment) | Separate design review |

#### M0b — `crates/shortwin-math` v0.3 → v0.4.1 (R3A-3, R4A-6; ECON "Removed in v0.4", "Added in v0.4" and changelog #74–#89)

Checked item by item against the crate as it stands (read-only, 2026-09-27): every delete / change item below exists in it; v0.4.1 (R4A-6) adds the items the v0.4 list missed and the round-4 changes (marked ★★).

**Delete**

- `persist::exec_run`, `persist::ema_run` and the Dutch-clock dating in `persist::liq_ema_leg` (its `secs` argument and `armed_at` result); `bounds::dutch_delta`; `ExitParams.{delta_liq_min_bps, delta_liq_slope_bps, delta_liq_max_bps}`; `observe::RingEntry.lo`.
- `persist::{ArmState, ArmParams, Armed, arm_update}` (replaced by `settle`, below); `ArmParams.settle_persist_s` and every `liq_persist_s` argument.
- `risk::{fv_ok, fv_gate, refuted, settle_kind}` in their raw (single-point) forms.
- `EmaParams.tau_s` and its `[5, 60]` validation (τ becomes `EMA_TAU_S = 5`), ★★and the `tau_s` parameter of `bounds::kappa_eff(g_bps, tau_s, params)` together with `liq_reference`'s use of `ema_params.tau_s` (both read the constant).
- ★★`ExitParams.liq_persist_s` (a struct field in the crate, not only an argument) and `ExitParams.{delta_liq_min_bps, delta_liq_slope_bps, delta_liq_max_bps}`.
- ★★`risk::{s1_legs, s2_legs}` in their single-point forms (`s1_legs(budget, fresh, b_live, b_rs, b_ema)`, `s2_legs(fresh, σ_live, σ_rs, σ_ema, σ_max)`): the legs are the box forms of `settle` (the EMA part at `(P_lo, L_hi)`).
- ★★`observe::RingEntry.{obs, lobs}` and `observe::obs_median` (the box's medians are over the bucket extremes, R4B-2).
- ★★`observe::switch_liquidity` (per-entry `min(e.l, l_new)`, `(x, y)` absent): replaced by `rewrite_liquidity` below.
- `pool::{recycle, touch_readonly}`, `Pool.unassigned_sol` and its genesis credit; the WASM facade's `unassignedSol`.
- The U256 square root in `quote::liquidity` (use `x.checked_mul(y)` and `sqrt_u128`; ECON §2.2 bounds `X·Y < 2¹²⁸`); `num::sqrt_u256` leaves the on-chain paths (tests may keep it).
- All test vectors generated from the v0.3 sims (regenerated below).

**Change**

- `quote::{reprice, reprice_token, curve_reprice}`: `sp = isqrt⁺(P)`, `X = ⌊L·10⁹/sp⌋`, `Y = ⌈L·sp/10⁹⌉` (R3B-7 c).
- `fees`: every fee resolves to a one-way rate in **ppm** (pump bps × 100; AMM v4 `⌈num·10⁶/den⌉`; CPMM `t + c`); gates compare ppm with `cap_bps·100`; `oneway_bps` is display-only (R3A-3 iii). Hypothetical-state quotes take the live fee tier (R3B-7 b; closes the crate's open question). ★★`FeeBps::capped`: when `φ_live > cap`, the risk quote charges **one combined fee at the cap**, `net + ⌈net·cap_bps/10⁴⌉` (one ceiling; the sims' `Venue._risk_fees`), not the crate's lp/protocol/creator split with three ceilings (up to 2 lamports apart); below the cap the three components are unchanged. Raydium risk quotes gross up at `min(rate, cap)` as now.
- `caps::{fee_size_cap, fee_oi_cap}`: `⌊cap_x100·φ_ppm·X/(100·10⁶)⌋`. `caps::conservative_x0`: `isqrt⁺(P_ema)` in both denominators, Raydium bound `⌊D·10⁹·10⁴/(λ_min_bps·isqrt⁺(P_ema))⌋`. `caps::max_total_q`: the sim's integer form `X − isqrt(⌊X²·10⁴/(10⁴ + ι)⌋)` exactly (the sim is normative; this settles the crate's owner note — the difference from the real-valued inequality is at most one raw unit).
- `pool_select::locked_depth`: `λ_bps = ⌊(counter − supply)·10⁴/counter⌋`, `D = ⌊λ_bps·isqrt(X·Y)/10⁴⌋` (ECON §2.1; v0.3's single floor on the raw ratio differs). `tier`: locked SOL `⌊D·isqrt(P_ema)/10⁹⌋`, `R_eff` term `⌊locked·10⁴/λ_min_bps⌋`.
- `observe::integrate`: ★★pushes `(ts, p, l, x, y, lo, hi, llo, lhi)` with `(x, y) = reprice(l, p)` for every venue (on a curve market these are the virtual reserves `(vT, vQ)`; `curve_reprice` applies the curve's offsets at quote time, so `integrate` takes no venue-kind argument) and `(lo, hi, llo, lhi)` the completed bucket's extremes (the clamped `obs`/`lobs` still drive the EMA step but are not stored), and returns the pre-roll gap (for `CoverageGap`). `EmaParams` bounds: `dtmax_s ∈ [1, 2]`, `clamp_bps ∈ [5,000, 10,000]`. ★★`ring_medians(&Ring) -> (P_mlo, P_mhi, L_mlo, L_mhi)`: lower median (index `⌊(n − 1)/2⌋`) of `lo`/`llo`, upper median (index `⌊n/2⌋`) of `hi`/`lhi`.
- `observe::reset_liquidity` and ★★`observe::switch_liquidity` → one `rewrite_liquidity(ema, ring, bucket, l_new, kind)`: graduation `L_ema = L_slow = l_new`; ★★Raydium (adoption or fast path) `L_ema = min(l_new, L_ema)`, `L_slow = min(l_new, L_slow)`; then for **every** entry `l = llo = lhi = L_ema` and `(x, y) = reprice(L_ema, p)` recomputed, **and the bucket cleared** (the sims' `_rewrite_ring_liq`; the crate's per-entry `min(e.l, l_new)` with `(x, y)` left stale differs and is replaced).
- `persist::liq_ema_leg(ring, below, n) -> bool` over the newest `n` entries' cached `(x, y)`.
- `bounds::{liq_reference, expiry_reference}` return `P_ref` (no δ); ★★`liq_reference` adds the cap `⌊lo_x·(10⁴ + κ_eff)/10⁴⌋` (R4B-2); ★★`bounds::max_in` splits into `max_in_liq(b_ref, δ, v, k_s, ert) = min(⌊b_ref·(10⁴ + δ)/10⁴⌋, v − k_s − ert)` (no reserve, R4A-1/R4B-1) and `max_in_exp(b_ref, δ, ε, v, k_s, r, π_ref, ert)` (rent and fee reserve).
- `waterfall::liquidate(…, k_s)` adds the keeper floor `kp + min(max(0, k_s − kp), W)`; ★★`waterfall::settle(v, kind, k_s, fv, q, p_ema, r_due)` returns the vesting lump `x − min(x, ⌊q·p_ema/WAD⌋ + r_due)` for S2/S3 and 0 for S1 (R4B-6; the v0.4 form `x − min(x, B̂_FV + R_due)` needed `B̂_FV`, which the sim reconstructed from FV with an off-by-one, R4A-5); `waterfall::expire` unchanged (the fee's lender share vests in `pool`).
- `rent::Aggr::{add, sub}(q, terms, rent_protocol_bps)` fold `(10⁴ − p)`; `rent::lender_credit(&Aggr, δJ)` loses its global-share argument; `PosRent` gains `rent_protocol_bps` and `credited_for` uses it; `rent::accrue` clamps `P̃ = min(P_ema, P_ACC_MAX)`.
- `risk::margin_d4`: `b_ema` is the quote on the newest ring entry's cached `(x, y)` (R3A-12).
- `pool` = `pool_model.py` v0.4.1: `Pool.{max_release ★★(was max_deadline), vest_bal, vest_last, vest_end, unmatched, lock_cal ★★, wipe_swept ★★}` and a protocol sink for credits with `S == 0` and no live wipe; `Lender.lock_until`; `touch` includes `_to_pot` and clears the lender's own expired lock (★★no aggregate change) and scales `lock_term` at epoch conversion; ★★`cal_advance` drops expired day buckets (prologue of every lock read or write, incl. `resolve`); `claim` draws the pot; `deposit` ★★requires the post-wipe sweep while a wipe record is live, clears `wipe_live`, then re-touches; `withdraw` sets `lock_until = ⌈(max(now, max_release) + lock_grace_s)/86,400⌉·86,400`, ★★moves a live lock's term between buckets, floors the term, and releases the vest to the last lender only when `shares == S`; `locked_total` ★★floors; `borrow(q, ★★release_at, p)`; `repay(rent_paid, lump)`; `settle(sol_in, lump)`; `maybe_roll` flushes the vest; ★★`vest` uses the value-weighted end; `sweep` vests (★★and sets the wipe bits while `S == 0`); `cover` adds to `real` (closes the crate's `cover_deficit` note).
- Test-vector generators (`tests/vectors/*/gen_*.py`) re-pointed at the v0.4.1 sims with their default switches: the ring tuple has ★★9 program fields (the sim's two trailing comparison fields are not exported), `Params` lost six fields in v0.4 and gained only analysis switches in v0.4.1, `Position` carries `p_rent` and the effective `ert`; the settlement generator `gen_settle.py` must cover the box decisions and arming (S1, S2, S3, refutation, disarm) ★★including push-then-freeze, consecutive attacker-only seconds and the `p'` veto (the attack traces and the v0.4.1 exit fuzz with its noisy and freezer runs), and `gen_exits.py` the S2/S3 waterfalls, FV and the risk predicates; `gen_pool.py` adds governance changes of `rent_protocol_bps`, the liveness wind-down, the round-3 cases of `attacks/pool_r3.py` and ★★the round-4 cases (`stale_lock_demo`, `lock_fuzz` with the untouched-state I-P4, `lock_tail_demo`, `wipe_gate_demo`, `restart_grief_table`, `premium_jit_table`).

**Add**

- `settle` module: `SettleBox`, `box_of(&Ema, &Ring, now)` (EMA point after `RING_LEN` s without an integration), `s1_legs`, `s2_legs`, `refuted`, `not_liquidatable`, `amount_ok`, and `ArmState` + `evaluate` implementing §3.8's order exactly (this closes the crate's "arming protocol" owner note).
- ★★`settle::pprime(expired, v_eff, b_fv, l_bps, penalty_bps, expiry_fee_bps)` used at every evaluated corner (R4A-3); `settle::box_of` over `observe::ring_medians` (R4B-2).
- Constants `EMA_TAU_S`, `RING_LEN`, `P_ACC_MAX`, `P_OPEN_MAX`, `COVERAGE_GAP_S`, ★★`LOCK_BUCKET_S`, `LOCK_BUCKETS`.
- `pool::{vest, release}`; ★★`pool::{cal_advance, cal_add, cal_sub, lock_until_for, allowance_view}` (the last for T-POT's untouched-state I-P4).
- `params::{ParamSet, validate}` = `shortwin_sim.validate_params` (compiled bounds and relational checks), shared by the program, the keeper, the WASM facade and T-CONFIG; since the M1 doc sync the sim checks every ECON §15 bound of the parameters it models (the penalty split's parts and the per-tier bounds included), and `params::validate_econ` carries the bounds of the parameters it does not model (§2.3.3).
- `tests/bench/cu_math` (Mollusk) with the §6.1 per-primitive budgets (T-CU-MATH). Optional: a bit-identical u64 fast path in `mul_div_u64`.

**Carried-over model notes** (implement exactly as `pool_model.py` v0.4.1 and keep the pinned vectors): (1) the lock term `⌊lock·WAD/Π⌋` can exceed u128 after deep reductions because Π resets only at `ΣB_Q = 0`, refusing a withdrawal (`Overflow`) until then; Π telescopes to `≈ WAD·ΣB_Q/b_min`, so this needs an open inventory below ≈ 5·10⁻¹⁴ of `b_min` (fewer than ≈ 53 raw units against a 10¹⁵ supply), which `min_notional` rules out below a price of ≈ 1.9 mSOL per raw unit; a proposed fix is to roll the Π epoch whenever `lock_norm` reaches 0; (2) **fixed in v0.4.1**: v0.4's ceiled `locked` could exceed `Σ lock_i`, which let two one-share lenders each draw all of `T` (the reviewers' lock-heavy fuzz found it); terms and the aggregate are now floored; (3) **fixed in v0.4.1**: rolls scale every calendar bucket and each lender's `lock_term` at conversion; (4) if the last shares can leave while dust is borrowed (`S = 0 ∧ ΣB_Q > 0`), a continuous rent credit reaches the protocol sink before any SOL exists, which the program's u64 `lender_sol` would refuse; the proposed rule is that the last shares cannot be withdrawn while `ΣB_Q > 0`. Neither (1) nor (4) is reachable by the v0.4.1 fuzz or by the round-3 and round-4 attacks.

**Owner gates (C0-11, V1-4, R3A-8).** D1–D4 are owner decisions in SPEC v0.3, so M2 (settlement with the D3 cap and D4 margin), M3–M4 (Raydium) and M6 (PumpSwap) build on decided ground in their SPEC-literal reading. The O1/O2 alternatives are **not built at all** (no parameters, no bounds; reserved bytes only). The decisions of §10 gate milestones as follows: **O6** before the route-broken flag and the extras are built (M2 at the earliest); **O16** before M1 (lender accounting); **O15** and **N22** before M2 (FV reading, keeper floor); **O13** before M3 (tier rule; the conservative default is built meanwhile); **O17** before M7 (keeper budget, and whether `coverage_authority` is enabled); **O9**, **O18**, **O19** before M11.

---

## 10. Open decisions for the owner

D1–D4 are decided (SPEC v0.3). The adopted first-pass defaults Q4–Q24 appear in §11 as parameters. Eight decisions were listed; SPEC §0 and §14 now record O6, O15, O16 and N22 as owner decisions (the O15 row states the recorded scope, S2 and S3). Each has a recommendation, which is also what the build does until the owner rules, and each needs a SPEC §14 entry (SPEC §15).

| # | Decision | Recommendation (= build default) | Why | Needed before |
|---|---|---|---|---|
| **O6** | Record the two new Emergency powers — the route-broken flag (A40) and bounded venue extras (A41) — as a SPEC §9 amendment | **Adopt** and record | Without them a venue interface change freezes every exit with no S3 (C0-4). Neither moves funds; the flag only refuses opens and starts the S3 chain; a bad extras set cannot brick exits (callers pick `none` or `previous`). They are not built until recorded (R3A-8 b) | building them (M2 at the earliest) |
| **O16** | Lender SOL: (a) continuous rent entitlement + MarketVault pot + 6 h lump vesting, or (b) one realised accumulator releasing all lender SOL over 6 h | **(a)** | (a) is SPEC §2.4-literal ("rent accrues to the pool continuously"), live (stranding 68.12 → 0.00002 SOL) and O(1). (b) is simpler, but a lender holding for 6 h takes 90 % of a position's rent and lump, against 90 % of the lump only under (a) (ECON §9.4) | M1 |
| **O15** | FV's `p'` for an expired, not CR-liquidatable position in **S2 or S3** | **The expiry fee — decided** (SPEC §0, §5.3, §14: S2 and S3) | SPEC §4: "a 2 % expiry fee instead of the full liquidation penalty". S2/S3 then pays lenders in SOL what the expiry would have paid them in tokens; the penalty reading moves 3–8 % of `B̂` to lenders (Tier A 1.82 SOL) and rewards racing an honest expiry with S3 (ECON §8.6). v0.4.1 decides "CR-liquidatable" on each box corner, so a contrary observation cannot switch the reading (R4A-3). The owner's record covers S2 as well as S3, as the sim (`fv_parts`), ECON §2.1 / §8.6 and the crate already did (ECON §13's exit fuzz pays one S2 of an expired position at the expiry fee); R3A-8 (a) is resolved | M2 (met) |
| **N22** | Liquidation keeper floor at `K_s`, paid from the borrower's remainder — or cap `K_s` instead (a SPEC §6.3 change) | **Floor** | Otherwise a third-party keeper earns more by letting a small position gap into S1, a lender loss. The top-up is ≤ what the borrower would lose in S1 and is zero above ≈ 1.4–5 SOL vaults (ECON §8.5) | M2 |
| **O13** | Age counted from listing for markets listed after graduation on AMM v4, PumpSwap and pre-`T_CLAMP` CPMM pools: Tier C for 24 h, at most Tier B until day 7 | **Accept** (UI explains the tier) | The pool's true age cannot be proven on-chain there; the error is conservative (higher margins, lower caps, shorter durations); markets listed before graduation are unaffected (R3B-12) | M3 |
| **O17** | Keeper operating budget per market, priority-price ceiling, which markets are covered, and whether to enable the pause-opens-only `coverage_authority` for automatic degraded mode | **Cover every market with open positions at targeted coverage** (≈ 0.003–0.03 SOL per market-day), **ceiling 100,000 µlamports/CU**, **enable the role** | Per-second coverage of every market costs 0.07–0.10 SOL per market-day batched, more than a 25 SOL beta market earns (0.005–0.04 SOL/day protocol rent); an uncovered market must pause opens visibly, and a human-paged pause is slower than the attack (ECON §5.8). The role moves no funds and can only pause opens (SPEC §11 gives pause power to a multisig) | M7 |
| **O18** | Confirm the PROPOSED parameters and venue rules as in §2.3.3 (merges v0.3 O3, O4, O5, O8): fee-scaled caps (3.0·φ / 7.5·φ); Raydium selection (λ_min 50 %, 25 % / 5 % margins, adoption only without open positions, fast path 120 s + 60 s, `pool_min_real_sol` 25 SOL; locker-LP pools stay list-only); venue fee caps 150 / 500 bps; oracle age 120 s; v0.4.1's re-derived bounds (settlement keeper terms at most their SPEC / ADOPTED values, persistence ≤ 3, SPEC CRs as per-tier minima) | **Accept** | Every corner of the core box passes the 1,655-run liveness battery, and every other liveness parameter passes at each compiled extreme alone and in a stress corner (ECON §5.7; the wording of v0.4, "every compiled corner passes", covered 7 parameters only, R4A-4); the fee caps make modelled attacks unprofitable on pump venues and bound Raydium residuals (ECON §6.1). Disclosed with the adoption rule: a standing minimum position can pin a market to a thinner pool; the remedy is an Emergency pause of opens (§2.6, R4B-10) | M11 |
| **O19** | Accept the disclosed residuals (merges v0.3 O12): the fixed-2 % sandwich (Tier A borrower −2.02 SOL at any wait); the held-model share of a liquidated borrower's remainder (Tier A +5.66 SOL) and the penalty and — on aged positions — rent the bought-down vault no longer covers (Tier A −2.06 SOL fresh, −8.02 SOL aged 25 d at 90 %), **or** buy the aged-position rent back with the rent-only reserve at the cost of honest liquidations lost in fast pumps (one extra S1 in 1,655 runs at default rates, more at higher rates; ECON §8.2); Raydium baskets (+0.5–8.5 SOL at the OI cap); the borrower's paid veto; the ±1 % settlement-amount push, the ≈ 13 s deferral per attacker-only second and the paid per-second widening deferral; 5 consecutive attacker-only seconds in degraded mode; an exiting lender's forfeited vest; SOL that arrives while a market has no lender going to the protocol; rebase-orphan dust | **Accept and disclose; keep no liquidation reserve** | Each lever that would remove one costs Tier C/D liveness (κ, τ, persistence, a liquidation reserve: ECON §5.7, §8.2) or SPEC fidelity; all are bounded and small next to gap risk | M11 |

**Standing items, not decisions for now:** gap-frequency calibration before mainnet is required (O9, ECON G37, M11); the run residual (up to ≈ 11 % of a runner's fair value, ECON §9.3) is accepted for the MVP with a withdrawal queue for v1.1 (O10); the no-route freeze of healthy positions is disclosed for the MVP, with a borrower-initiated SOL close (a SPEC §5.3 amendment) recommended for v1.1 (O14).

**Closed since v0.3:** O1/O2 (non-literal D3/D4 readings) are not built at all (R3A-8 c); O3, O4, O5, O8 are merged into O18 and O12 into O19; O7 (stranded cashback to lenders) is adopted and now vests; O11 (CPMM TWAP bound) is on the v1.1 list.

---

## 11. PROPOSED decision register (SPEC §15)

These go into SPEC §14 as "Delegated, proposed" after review. ECON's register (ECON §14: D1–D4, G*, N*) is adopted as written; OWNER items are in §10. Rows changed in v0.4 are marked ★; superseded rows are kept for traceability.

| ID | Decision | Parameters | Section | Origin |
|---|---|---|---|---|
| A1 | Venue state machine incl. Raydium; `None` = list-only and never terminal; D1 precedence | — | §2.5 | v0.1 D2 |
| A2 | Reject mayhem-mode and non-SOL-quote curves/pools; allow legacy SPL pump coins (ADOPTED Q6) | — | §2.5 | v0.1 D3 |
| A3 | Allowlist mint filter; entry-strict / exit-tolerant; `flag_unsafe_mint` (ADOPTED Q7) | — | §3.2 | v0.1 D4 |
| A4 | Vault topology; single per-market `trader` signer; delta post-conditions; the core program never forwards a user's signature to a venue | — | §2.3–2.4 | v0.1 D5 |
| A5 | Ephemeral PDA-address WSOL account; ATA fallback for PumpSwap per T-ATA | — | §2.4.1 | v0.1 D6 |
| A6 | Strict remaining-account parsers; every pricing/route/tier/guard account mandatory; denylist | — | §3.0 | v0.1 D7 |
| A7 ★ | Venue guard on opens for all venue programs incl. Raydium, as an **event-driven latch**: permissionless `flag_venue_upgraded` in `VenueGuard` (refreshes the recorded slot while flagged), `revalidate_venue(venue, expected_slot)` by Emergency; per-venue masks (curve incl. pAMM; each Raydium kind incl. both Raydium programs); no ProgramData in `open_short` (ADOPTED Q13; decision 4; R4B-7) | `VenueGuard` | §2.3.2, §3.1, §6.5 | v0.1 D8 |
| A8 | Timelocked config with compiled floor; compile-time venue ids; hard bounds on every parameter (ADOPTED Q23) | `timelock_s`, `MIN_TIMELOCK_S`, §2.3.3 bounds | §2.3, §3.1 | v0.1 D9 |
| A9 | Protocol fees, lender SOL, the pot and the lock state on `MarketVault`; permissionless sweep | — | §2.3.6 | v0.1 D10 |
| A10 | `OracleCache` + `crank_sol_usd`; stale → Tier C; exits never read it (ADOPTED Q8) | `oracle_*` | §3.9, §4.7 | v0.1 D11 |
| A11 | Tier liquidity `2·min(R_now, R_slow)` on real SOL (ADOPTED Q19) | `liq_slow_*` | §4.7 | v0.1 D12 |
| A12 ★ | `graduated_at` = first observation / listing; CPMM `open_time` lower bound; age from listing on AMM v4, PumpSwap and pre-`T_CLAMP` CPMM (ADOPTED Q18; O13) | `T_CLAMP` | §3.2, §4.7 | v0.1 D13 |
| A13 | Boost-window gate on dust + time window (ADOPTED Q15) | `boost_dust_lamports`, `boost_window_s` | §3.4 | v0.1 D14 |
| A14 ★ | Straddle via a ≥ 3-transaction Jito bundle, priced on the executed path `B̂_str` on live and EMA legs, keeper fallback, curve completion by the `complete-curve` helper (ADOPTED Q14; A61, A66) | `curve_oi_gamma_bps` | §4.3, §5.4 | v0.1 D15 |
| A15 | Keeper calls `migrate_v2` top-level in its own transaction (ADOPTED Q16) | — | §5.4 | v0.1 D16 |
| A16 ★ | Lender instructions accrue and roll only; claim / cover / sync never write `Market` | — | §3.0, §3.3 | v0.1 D17 |
| A17 ★ | EMA and slow-liquidity parameters market-level (forced adoption per A59); τ a compile-time constant; the kink market-level with **its own version**, adopted only with no open position (R3A-9) | `ema_dtmax_s`, `ema_clamp_bps`, `liq_slow_*`, `util_kink_bps`, `EMA_TAU_S` | §2.3.2, §3.0 | v0.1 D18 |
| A18 | Borrower paid via the position close | — | §5.2 | v0.1 D19 |
| A19 | Venue tolerance and measured accounting; `ert` = 0 on Raydium | `exec_tol_bps`, `exec_rent_tol` | §2.4, §4.3 | v0.1 D20 |
| A20 | `add_margin` (ADOPTED Q12) | — | §3.5 | v0.1 D21 |
| A21 | Per-market launch guard (ADOPTED Q21) | `market_max_notional` | §3.4 | v0.1 D22 |
| A22 | Separate `liquidate` / `expire` / `sol_settle`, conditions on-chain | — | §3.6–3.8 | v0.1 D23 |
| A23 | Per-venue venue predicate; `s3_no_route` with fee hard cap and route-broken flag; continuous S3 chain (amended by A50, A71) | `S3_OBS_GAP_S` | §5.3 | v0.1 D24 |
| A24 | PumpSwap `disable_flags`: any non-zero blocks opens; the T-DISABLE buy bit (constant) is no-route | — | §5.3 | v0.1 D25 |
| A25 | Close while liquidatable, no penalty (ECON G28) | — | §3.5 | v0.1 D26 |
| A26 ★ | Locked-at-open scope incl. `rent_protocol_bps`; what governance can still reach is listed, including the global hard cap in market-level predicates (ADOPTED Q9; V1-5, R3A-4, R3A-5) | — | §2.3.5 | v0.1 D27 |
| A27 | Market creation fee (ADOPTED Q24) | `market_creation_fee` | §3.2 | v0.1 D28 |
| A28 | v0 + protocol ALT; v1 optional | — | §6.4 | v0.1 D29 |
| A29 | Hand-parsed Pyth `PriceUpdateV2` | — | §3.9 | v0.1 D30 |
| A30 ★ | Zero-dependency `no_std` math crate with its own U256 (limb division), amended by A68 | — | §2.2 | v0.1 D31 |
| A31 | Hand-built CPIs; pump v2 instructions (legacy behind a flag); Raydium v2 tags only | `pump_ix_version` | §2.1, §3.12 | v0.1 D32 |
| A32 | S1/S2 paused during migration; S3 only | — | §3.8 | v0.1 D33 |
| A33 | Trader-state normalisation before every leg (C0-3, C1-3) | — | §2.4 | v0.2 |
| A34 ★ | Exit reference bounds with a **fixed** δ; the liquidation reference also capped at `lo_x·(1 + κ_eff)`; **a liquidation reserves nothing in `max_in`** (SPEC §5.2), an expiry keeps rent and the expiry fee senior; quoted fee bases, expiry keeper reward (ECON N1, G14, G15; C0-1; decision 1; v0.4.1: R4A-1, R4B-1, R4B-2) | `kappa_*`, `delta_liq_bps`, `delta_exp_bps`, `ref_max_age_s`, `exec_tol_bps`, `expiry_keeper_*` | §3.6, §3.7, §5.2 | v0.2 |
| A35 ★ | Observation buckets, ring with cached reprojection, liquidation persistence on the newest `liq_persist_n` entries, per-position settlement arming (ECON G12, G13; decisions 2, 3) | `ema_*`, `RING_LEN`, `liq_persist_n`, `settle_persist_n`, `settle_arm_gap_s` | §4.4, §4.5, §3.8 | v0.2 |
| A36 ★ | Rent basis indices with per-position locked rate curve and protocol share | — | §4.5 | v0.2 |
| A37 ★ | Pro-rata withdrawal with borrowed-inventory locks; Π decays below the running minimum of ΣB_Q; every lock expires at `lock_until` (ECON G33; C0-9; A57) | `lock_grace_s` | §3.3 | v0.2 |
| A38 ★ | Realised claims with per-lender memory, drawing the MarketVault pot up to the own deficit; no share withdrawal with negative pending; `cover_deficit` (ECON G22, N4, N17; C0-13; A56) | — | §3.3 | v0.2 |
| A39 ★ | Share epochs with `ShareEpoch` records; `sync_lender`; epoch jump; live full-wipe record; SOL with `S = 0` and no live wipe to the protocol (ECON N3, N19; C0-6; A58) | `R_MAX`, `REBASE` | §3.3, §3.8 | v0.2 |
| A40 | Emergency route-broken flag: opens refused and S3 evidence only; never a quote, exit or `pool_fail` input (C0-4, V2-1, V1-7; OWNER O6) | — | §3.1, §5.3 | v0.2 |
| A41 | Bounded, Emergency-set trailing venue extras: fixed keys or derivation templates, two selectable sets plus none, in `VenueExtras` (C0-4, V2-6; OWNER O6) | `VenueExtras` | §3.0 | v0.2 |
| A42 ★ | Raydium pool eligibility and selection by locked depth `D`; listing margin; non-fast adoption only with no open position; fast path on `pool_fail` with a collection window; opens sized on locked depth (OWNER O18; A60, A71) | `pool_*` | §2.6 | v0.2 |
| A43 | Venue fee caps: open cap; hard cap = no route for S3 and `pool_fail`; FV and risk-quote fee ceiling (OWNER O18) | `venue_fee_cap_bps`, `venue_fee_hard_cap_bps` | §4.5, §5.3 | v0.2 |
| A44 ★ | `claim_venue_rewards` and `sweep_trader`; stranded value to lenders, vested (C0-18, R3B-2) | — | §3.9 | v0.2 |
| A45 | Fee-recipient validation; PumpSwap venue-side accounts must pre-exist (C0-14, C1-8, C1-9) | — | §3.12 | v0.2 |
| A46 | Grief-proof PDA account creation (sweep, fund, allocate, assign) | — | §2.4.1 | v0.2 |
| A47 | Raydium sells via a delegate on `pool_vault`; buys deliver straight into it | — | §2.4 | v0.2 |
| A48 | `ert` funding before pump-venue sells (C1-4) | — | §2.4 | v0.2 |
| A49 | Test-only mock venue behind the `test-venue` feature, never in release builds | — | §9 | v0.2 |
| A50 | Separate route predicates: `venue_executable` (quotes, exits, spot leg, observation) and `s3_no_route` (S3 chain and opens only); every risk quote caps the fee at the hard cap (V2-1, V1-7; completed by A71) | `venue_fee_hard_cap_bps` | §4.3, §5.3 | v0.3 |
| A51 | *Superseded in v0.4 by A34's fixed δ*: the Dutch exit clock over executable ring entries (V1-1) | — | — | v0.3 |
| A52 ★ | Settlement arming reset only by box-robust evidence; one clock for S1 and S2; kind chosen at execution, S1 first (V2-4, V1-3, R3A-6; detailed by A65) | `settle_*` | §3.8 | v0.3 |
| A53 | *Superseded in v0.4 by A65*: the FV gate on S2/S3 (V2-9) | — | — | v0.3 |
| A54 | *Superseded in v0.4 by A7's latch*: the `VenueSlots` mirror (V1-11) | — | — | v0.3 |
| A55 | `VenueExtras` accounts per venue (A41's storage) (V2-6) | — | §2.3.2 | v0.3 |
| A56 ★ | Realised lender SOL allocated to shares on arrival (`acc_r`); surplus beyond entitlement moves to the MarketVault pot at a touch; claims pay `min(pending, real − claimed + pot draw, lender_sol)` (V2-2, R3A-2) | — | §3.3 | v0.3 |
| A57 ★ | Π decays only when ΣB_Q falls below its running minimum since the first lock of the Π epoch; locks expire at `lock_until = ⌈(max(now, max_release) + lock_grace_s)/1 d⌉·1 d`, `max_release` including each position's S3 tail (V2-3, R3B-1; R4A-8, R4B-8) | `lock_grace_s` | §3.3 | v0.3 |
| A58 | A lender converted to 0 shares jumps to the current epoch; SOL credited while S = 0 after a full wipe tops up the wipe record, which also receives the flushed vest (V2-12, R3B-2) | — | §3.3 | v0.3 |
| A59 | EMA-parameter versions reach markets with open positions after `ema_force_delay_s`; the kink never does (V2-13, R3A-9) | `ema_force_delay_s` | §3.0 | v0.3 |
| A60 ★ | Raydium opens sized on locked depth / `λ_min` (no `λ_now` gate); listing margin; adoptions inside a window do not extend it; best-candidate fast-path window (V2-7, V2-8) | `pool_list_margin_bps`, `pool_fast_window_s` | §2.6, §4.6, §4.7 | v0.3 |
| A61 | Straddle quote `B̂_str` = the post-migration pool buying all of `Q`; keeper fallback without tx3 (V1-9) | — | §4.3, §5.4 | v0.3 |
| A62 | Buy legs accept over-delivery (surplus to `pool_vault`); one sell-leg baseline and net `S` on both pump venues (V2-10, V1-13) | — | §2.4 | v0.3 |
| A63 | A reader failure on a correctly keyed venue-state account means "not executable", never a revert, for `crank_price` and `sol_settle` (V1-8) | — | §3.0, §4.1 | v0.3 |
| A64 | No observation is recorded while the venue cannot execute (except `PumpMigrating`); `SolSettle` reports the EMA's age (V2-5) | — | §3.0, §4.4 | v0.3 |
| A65 | **The settlement box**: every settlement decision (arm/keep, refute, not-liquidatable, kind, amount) must hold on the box spanned by `(P_ema, L_ema)` and the ring medians **of the bucket extremes** (lower median of the lows, upper median of the highs; v0.4.1); FV's `p'` is decided on each corner; amounts within `fv_dev_bps`; the box collapses to the EMA point after `RING_LEN` s without an integration (R3A-1, R3A-6, V2-9; decision 5; R4B-2, R4A-3) | `fv_dev_bps` | §3.8, §4.5 | v0.4 |
| A66 | `complete-curve` helper program for the straddle bundle: reads `rT` at execution, no-op if complete (R3B-11) | — | §5.4 | v0.4 |
| A67 | Zero-copy layouts use `PodU128`/`PodI128` with explicit padding; compile-time size and offset asserts on sbpfv3, x86_64 and wasm32; math types never cast from account bytes (R3B-4; decision 9) | — | §2.3 | v0.4 |
| A68 | Numerics: ECON §2.1 normative; u128 fast path first, Knuth-D fallback only; 256-bit only for ECON §2.2's products; no U256 square root; `P_ACC_MAX`, `P_OPEN_MAX`; per-primitive CU budgets (R3B-3, R3A-12; decision 8) | `P_ACC_MAX`, `P_OPEN_MAX` | §2.2, §6.1 | v0.4 |
| A69 | Keeper duties with batching and observation counting; degraded mode: `CoverageGap` event, opens paused (`KeeperCoverage`), optional pause-opens-only `coverage_authority` (R3B-5; decision 10; OWNER O17) | `COVERAGE_GAP_S`, `coverage_authority` | §3.1, §7.1 | v0.4 |
| A70 | Lump vesting over `lump_vest_s` on `Market` with a value-weighted end, including the part of an S2/S3 take above `⌊Q·P_ema/WAD⌋ + R_due`; deposits into a live wipe record wait for the post-wipe sweep; the lock state on `MarketVault` so claims run the full touch without writing `Market` (R3B-2, R3A-2; R4B-5, R4B-6, R4A-7) | `lump_vest_s` | §2.3.6, §3.3 | v0.4 |
| A71 | `pool_fail` predicate and its own chain `pool_fail_since` / `last_pool_fail_obs`; market-level predicates use the global hard cap (R3A-4, R3B-8) | — | §2.6, §5.3 | v0.4 |
| A72 | Every venue change rewrites the ring's `l`, `llo`, `lhi` and `(x, y)` and clears the bucket (R3B-7 a, R3A-11 d) | — | §2.5, §2.6, §4.4 | v0.4 |
| A73 | Governance proposals pass `validate_params` (compiled bounds + relational checks) and the `config_check.py` liveness battery (1,655 runs incl. per-second noise, the genuine-pump grid and aged positions since v0.4.1); τ compiled; every liveness parameter's bounds re-derived on it (R3B-6; R4A-4) | — | §2.3.3, §3.1 | v0.4 |
| A74 | Liquidation keeper floor at `K_s` from the borrower remainder (R3B-10; OWNER N22) | — | §3.6, §5.2 | v0.4 |
| A75 | Lock expiry calendar on a zero-copy `MarketVault`: 128 day buckets of lock terms keyed by `lock_until`; the aggregate drops a bucket when its day ends; lock terms floored; I-P4 asserted on the untouched state (R4A-2, R4B-3) | `LOCK_BUCKET_S`, `LOCK_BUCKETS` | §2.3.6, §3.3 | v0.4.1 |

---

## 12. UNVERIFIED items (carry into test gates)

| # | Item | Gate |
|---|---|---|
| U1 | Pump `buy_v2` and PumpSwap `buy` with a non-whitelisted PDA `user` under CPI (System debits, venue-created accounts). Sells are verified on mainnet (C1-7) | T-PDA-USER |
| U2 | PumpSwap accepts a non-ATA WSOL account owned by `user` (the IDL has no ATA constraint [V11]); pump/pAMM base-ATA requirement | T-ATA |
| U3 | Bit order of PumpSwap `disable_flags` (docs: "Currently, it is not used") | T-DISABLE (hard gate) |
| U4 | Whole-instruction CU, trace and size figures (§6). The math part of §6.1 is measured [V18]; runtime overhead is estimated | T-CU, T-TRACE, T-SIZE |
| U5 | CU / latency / fee cost of copying a 10 MiB ProgramData into the input region — now only in `flag_venue_upgraded` and `revalidate_venue` | T-CU |
| U6 | Which pump `sell_v2` rent items debit `user` before proceeds are credited; whether PumpSwap `sell` creates fee ATAs (C1-4) | T-RENT |
| U7 | Whether PumpSwap creates missing ATAs through the ATA program (depth 4) | T-DEPTH |
| U8 | The ≥ 3-transaction straddle bundle with `complete_curve` (CU, size, trace, the trade-before-bundle case) | T-STRADDLE |
| U9 | `claim_cashback` without the user's signature (IDL [V11]) and its proceeds path | T-CASHBACK |
| U10 | Raydium: delegated `pool_vault` as sell source; direct delivery into `pool_vault` for Token-2022 CPMM pools | T-RAY-PDA |
| U11 | CPMM exact-out with the creator fee on the output side (source only) | T-RAY-MATH |
| U12 | Deployed Raydium binaries are byte-identical to `raydium-amm@d26944b` / `raydium-cp-swap@59fb845` (behavioural match only); no verified builds | disclosed; T-RAY-MATH on the dumped ELFs |
| U13 | Whether Raydium's admin has ever disabled pools or changed fees historically (none disabled today) | keeper monitor |
| U14 | Which CPMM deploy first contained the `open_time` clamp (hence `T_CLAMP` = 2024-06-01) | conservative constant |
| U15 | The proportional boost rule (source closed) and what raises VQR after the boost on some pools | read VQR live; ECON §10.1 |
| U16 | The 24 M CU per-writable-account limit in validator clients other than Agave (C0-17) | T-CASCADE, monitoring |
| U17 | CPI'd memo satisfies Token-2022 MemoTransfer | T-MEMO |
| U18 | Anchor 1.2.0 `declare_program!` on the vendored IDLs; which Agave version the lockfile resolves (C1-14; for Mollusk's `^4.2.0` resolver 3 picks 4.3.0, which Mollusk 0.15.1 does not compile against — M0b, §1.1; the program tests' pin is M1's); solana-verify vs `anchor build` hash equality | M0 |
| U19 | Surfpool Pump template fidelity after the 2026-09-23 pump deploy | M5 fork E2E |
| U20 | Real-world arbitrage of 1–3 s held pumps (ECON §12.6 residuals assume none) | calibration (O9, O19) |
| U21 | Gap frequency and tail parameters (ECON §11.6) | calibration from M8 data (O9) |
| U22 | Jito bundle semantics as documented (≤ 5 transactions, sequential, all-or-nothing) | T-STRADDLE |
| U23 | The Switchboard shutdown (secondary sources only); not used either way | — |
| U24 | Whether pump and PumpSwap reject unexpected trailing accounts (the reason `none` and the previous extras set stay selectable, A41) | T-REPLAY-PDA on the dumped ELFs |
| U25 ★ | How many `crank_price` instructions fit one transaction: byte estimate 4 (PumpSwap / CPMM) or 5 (curve) per v0 transaction with the protocol ALT, up to 9 per v1 transaction (v0.4.1, §6.4, R4B-9); and that v1 transactions land for the keeper's batches | T-SIZE |
| U28 ★★ | The per-second price noise the ECON §5.7 battery assumes (±0.3 %, ±1 % between a live quote and the one-second-old reference) — a modelling assumption, not a mainnet measurement per tier | keeper telemetry (M7): distribution of `B̂_live/B̂_ref` at liquidation time |
| U26 ★ | The latch's reaction time: the keeper's Yellowstone stream sees a ProgramData change and lands `flag_venue_upgraded` in the next slot | T-LATCH on a fork; monitoring |
| U27 ★ | ECON §5.8's keeper cost inputs (CU per instruction, priority prices) | M7 measurement |

---

## 13. Sources

### 13.1 Document keys

| Key | Document | Primary sources it cites |
|---|---|---|
| SPEC | `docs/SPEC.md` v0.3 | — |
| ECON | `docs/design/ECONOMICS.md` **v0.4**, `sim/pool_model.py`, `sim/shortwin_sim.py`, `sim/config_check.py`, `sim/keeper_cost.py`, `sim/attacks/*.py` (incl. `fixed_delta.py`, `refute_push.py`, `fv_push.py`, `venue_switch.py`, `pool_r3.py`, `cascade.py`), `sim/out/` | pump-public-docs@81091419; `@pump-fun/pump-sdk@2.0.0`, `@pump-fun/pump-swap-sdk@1.20.0`; `raydium-io/raydium-cp-swap@59fb845a`, `raydium-io/raydium-amm@d26944bf`; mainnet T1–T12, A1–A7 |
| R-PUMP | `docs/research/pumpfun-bonding-curve.md` (incl. Review corrections) | `pump-fun/pump-public-docs@81091419e4457566469d4e2a27f64ed84d42419c` (IDL `e0687ae9`); program `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P`; txs `5zNHaL6f…`, `535njpqh…`, `3u74vnKG…`, `SfdLRmJp…`, `zsGhkQcr…`; agave@f7716596 |
| R-VEN | `docs/research/graduated-venues.md` (incl. Review corrections) | same docs; `pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA`; TX-MIG `4e9yHCpw…`, TX-BUY `udGBnNtp…`, TX-SELL `2oSvEbDT…`, `4dJY8x1e…`; Pyth docs and feed `7UVimffx…` |
| R-RAY | `docs/research/raydium-amm.md` | `raydium-io/raydium-amm@d26944bfb76fb5fa8f91e5d440c2050ed358ef81`; `raydium-io/raydium-cp-swap@59fb845a9e5bb569c8b2f3415f13b0c0ebcc6b92`; CPMM PMP IDL `FHuJXL7c…`; `solana-program/token@8185db13`; agave@f7716596; the TX-V4-* / TX-CP-* transactions; OtterSec verify API |
| R-JUP | `docs/research/jupiter-cpi.md` (incl. Review corrections) | `JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4` + IDL `C88XWfp2…`; `jup-ag/docs@16e9e9d5`; agave@f7716596 |
| R-TOOL | `docs/research/toolchain-and-layout.md` (incl. Review corrections) | `otter-sec/anchor@v1.2.0` (`84a63f9f`) and `@78ed411a40`; solana-verify v0.5.2; LiteSVM v0.16.0; Mollusk 0.15.1; Surfpool v1.6.0; Squads-Protocol/v4@af94153 |
| R-MINT | `docs/research/mint-safety-filter.md` | `solana-program/token-2022@23b03f88`; interface@v2.1.0; ATA@4e58a109; agave `system_processor.rs`@f7716596 |
| CRIT | Design-review findings `C0-1`…`C1-15` (scratchpad `critics.md`), second-round `V1-1`…`V2-14` and third-round `R3A-1`…`R3A-13`, `R3B-1`…`R3B-12` with the reviewers' experiment scripts (all 2026-09-27) | as cited per finding |
| ARCH tools ★ | `docs/design/arch/layout.py` (the §2.3.4 layout), `docs/design/arch/cu_budget.py` (the §6.1 math budgets), `docs/design/arch/cu_math/` (the SBF micro-benchmark behind V18) | this document |

### 13.2 Checks run for this document

| # | What | Result | Source |
|---|---|---|---|
| V1 | (v0.1) ProgramData deploy slots at slot 450,811,029 | Superseded by V9 (unchanged) | mainnet RPC |
| V2 | Stack heights in top-level `migrate_v2` tx `3u74vnKGX9XmZRViv3qCvhfDQUUY7SEZtyrozhCfPUBaHVk1ZNr62vVnPAVTW8sFzVMrz7e1zco5piqDRdHp7yUd` | Max inner `stackHeight` = 4 (pump → pAMM `create_pool` → ATA → Token-2022/System) | mainnet `getTransaction`; limit 5 per agave@f7716596 `program-runtime/src/execution_budget.rs` |
| V3 | Raydium-graduated pump coin Fartcoin `9BB6NFEcjBCtnNLFko2FqVQBq8HHM13kCyYcdQbgpump` | Curve `TBHe5tJnuT4CQbHorJ1uVdfUoaYGPKgfCpiv2jgesVN`: complete, all reserves 0; canonical PumpSwap pool `EVtHHoEb2V35V1fpzUZcaASgFn6N4yGWoaZ1twNDVmjW` does not exist; its main SOL pool is AMM v4 `Bzc9NZfM…` (38,100 SOL, λ 99.78 %) [R-RAY §6.4] | mainnet RPC |
| V4 | When rent state is checked | Once per transaction (`TransactionAccountStateInfo::new_pre_exec` / `new_post_exec` + `verify_changes` around `process_message`) | agave@f77165963c709934e8ea8564187c00f081b597d1 `svm/src/transaction_processor.rs` L1144–1205 |
| V5 | Rent-exempt minimums (Rent sysvar 5,080 lamports/byte-year, threshold 1.0) | 0 B = 650,240; 137 B = 1,346,200; 165 B = 1,488,440; 170 B = 1,513,840 lamports; i.e. `(len + 128)·5,080` | mainnet `getMinimumBalanceForRentExemption` [C1 verified] |
| V6 | PumpSwap `GlobalConfig` layout and `disable` semantics | `disable_flags` u8 @56 (0 today), `creator_fee_configurable` @940, 949 B; errors 6020/6021; docs: "`disable_flags` is a bitmask … Currently, it is not used" | pump-public-docs@81091419 `idl/pump_amm.json`, `docs/PUMP_SWAP_README.md` L58 |
| V7 | Pyth SOL/USD shard-0 `7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE` | Owner `rec5EKMG…`, verification Full, feed `ef0d8b6f…`. **Corrected (C1-12):** 55 s heartbeat or 0.5 % move; publish-time steps 52–54 s; staleness up to ≈ 45 s — not "within ~10 s" | mainnet RPC; Pyth docs [R-VEN Review corrections 2] |
| V8 | Latest releases (2026-09-27) | Anchor v1.2.0, LiteSVM v0.16.0, Surfpool v1.6.0, Agave v4.3.0 (4.4.0-alpha.5 pre-release) | `gh release list` |
| V9 | ProgramData deploy slots, `getMultipleAccounts` at context slot 450,850,312; re-read with `getAccountInfo` (program → ProgramData → tag 3, deploy slot) at context slot 450,944,934 for v0.4.1: all five unchanged | pump `B5MvUwXd…` 449,734,335 (2026-09-23T14:45:19Z, 10,485,760 B); pAMM `6naEzKeU…` 446,462,733 (09-12, 10,485,760 B); pfee `75Uu23mq…` 446,465,969 (948,989 B); AMM v4 `A7ZG7ByD…` 445,763,204 (2026-09-10T01:52:56Z, 1,406,429 B, authority `FytDrVz…`); CPMM `DMawCQzb…` 445,763,504 (01:54:31Z, 793,869 B, same authority); Token-2022 `DoU57AYu…` 427,147,035; Jupiter `4Ec7ZxZS…` 449,280,566. All unchanged vs. the research notes | mainnet RPC (`api.mainnet-beta.solana.com`) |
| V10 | Per-writable-account block CU limit | `MAX_WRITABLE_ACCOUNT_UNITS = 24_000_000` (L33), not feature-gated; `MAX_BLOCK_UNITS` 60 M (SIMD-0256) / 100 M (SIMD-0286) | `anza-xyz/agave@f77165963c709934e8ea8564187c00f081b597d1` (master HEAD 2026-09-26) `cost-model/src/block_cost_limits.rs` |
| V11 | Pinned pump IDLs | pump `claim_cashback` (`253a237ebe35e4c5`): `user` writable, **not signer**; pAMM `claim_cashback`: `user` writable, not signer, `user_wsol_token_account` "may be any token account of `quote_mint` owned by `user`"; pAMM `buy`/`sell` `user_base_token_account` / `user_quote_token_account` carry **no ATA seed constraint** (the fee-recipient and creator-vault token accounts do) | `pump-fun/pump-public-docs@81091419` `idl/pump.json`, `idl/pump_amm.json` |
| V12 | Delegate clearing | Token-2022 `process_transfer`: a delegated transfer decrements `delegated_amount` and sets `delegate = None` at 0 (L484–495); p-token does the same [R-RAY §4.2] | `solana-program/token-2022@23b03f88e44a9ef2587e5a08206a09eca0c8f17b` `program/src/processor.rs` |
| V13 | CPMM admin fee bounds (V2-1) | `update_trade_fee_rate` / `update_creator_fee_rate` only `assert!(trade_fee_rate + creator_fee_rate < FEE_RATE_DENOMINATOR_VALUE)` (10⁶ ppm), i.e. a config fee of 5.01 % (or 99 %) is accepted | `raydium-io/raydium-cp-swap@59fb845a9e5bb569c8b2f3415f13b0c0ebcc6b92` `programs/cp-swap/src/instructions/admin/update_config.rs` L47–60 (`gh api`, 2026-09-27) |
| V14 | AMM v4 admin key and fee retention (V2-5, V2-7) | Mainnet build (`not(any(testnet, devnet))`): `config_feature::amm_owner = GThUX1Atko4tqhN2NaiTazWSeFWMuiUvfFnyJyUghFMJ`, checked as signer by the admin instructions; `process_swap_base_in_v2` transfers the full `amount_in` (fee included) into the pool vault | `raydium-io/raydium-amm@d26944bfb76fb5fa8f91e5d440c2050ed358ef81` `program/src/processor.rs` (config_feature, `process_swap_base_in_v2`) |
| V15 | Fartcoin's Raydium SOL pools (V2-5) | 9 pools (2 AMM v4, one per side ordering; 7 CPMM); only `Bzc9NZfMqkXR6fz1DBph7BDf9BroyEf6pnzESP7v5iiw` is eligible (status 6, 38,176 SOL real, λ 0.9978); the others hold ≤ 5.2 SOL or have λ < 0.01 | mainnet `getProgramAccounts` (memcmp on the mint offsets) + `getMultipleAccounts`, 2026-09-27 (re-run of the reviewer's script) |
| V16 | Locker coverage of creator-fee CPMM pools (V2-11) | 1,668 CPMM pools with `enable_creator_fee` and WSOL as token 0; 311 with ≥ 25 SOL real; 244 of those with λ ≥ 50 %, 67 below (e.g. `AtqUb7DhVjiWgUhkrE7CnLGD9aDDedhSNrf15iUwHFGk` 2,024.6 SOL λ 0.0000) | mainnet `getProgramAccounts`, 2026-09-27 (re-run) |
| V17 | Strict entitlement-side solvency (V1-10) | On the unmodified v0.2 `pool_model.py`, the fuzz with the model's own seed 20260927 violates `Σ max(pending,0) + unassigned ≤ sol_vault + receivable` 6 times, by up to 333,507,609 lamports (seeds 1–3: 0 violations) | `sim/pool_model.py` v0.2 re-run (scratchpad) |
| V18 ★ | Per-primitive CU of `shortwin-math` on SBF | Built with `cargo-build-sbf 4.1.0 --tools-version v1.57 --arch v3` (platform-tools v1.57, rustc 1.95) from a read-only copy of the crate as built for M0 (2026-09-27), run on `solana-test-validator` 4.1.2 via `simulateTransaction` (`unitsConsumed`, 50 iterations minus 0, loop overhead subtracted). u128 `mul_div` 691; 256-bit (Knuth D) `mul_div` 783; `mul_div_u64` 267; native u64 76; PumpSwap buy quote 1,452; CPMM 1,872; AMM v4 959; `reprice` 843; `isqrt(X·Y)` 734 (u128) / 624 (U256); `rent::due` 1,287; `credited_for` 2,637; `accrue` 1,109; one EMA roll 2,990; `obs_median` 1,604; `sigma_xy` 1,473; EMA-leg entry 2,289 (v0.3) / 1,468 (v0.4 cached). Agrees with the reviewer's independent bench where they overlap (u128 mul_div ≈ 622, PumpSwap quote ≈ 1,445) | `docs/design/arch/cu_math/` |
| V19 ★ | Agave compute costs | `DEFAULT_INVOCATION_COST = 946` (CPI), `create_program_address_units = 1500`, `cpi_bytes_per_unit = 250`, `MAX_INSTRUCTION_STACK_DEPTH = 5`, `MAX_COMPUTE_UNIT_LIMIT = 1,400,000` | `anza-xyz/agave@f77165963c709934e8ea8564187c00f081b597d1` `program-runtime/src/execution_budget.rs` L8–233 (`gh api`, 2026-09-27) |
| V20 ★ | `u128` alignment per target (R3B-4) | Compile-time asserts: `align_of::<u128>()` = 16 and v0.3's `RingEntry` (i64 + 5 × u128) = 96 B on x86_64 and wasm32 (rustc 1.98.1); = 8 and 88 B on `sbpfv3-solana-solana` (platform-tools v1.57 rustc 1.95) | local `rustc --target` builds, 2026-09-27 |

---

## Appendix A. Review resolution

### Review resolution — round 4 (fourth review, `R4A-x` / `R4B-x`)

One row per finding of the fourth 2026-09-27 review. Every finding was re-checked before it was fixed: the reviewers' scripts (`scratchpad/r4/resolution/exp/*`, `scratchpad/r4/fresh/sim/*`) were re-run against the v0.4 sims and reproduced, then against the v0.4.1 sims; the new or changed evidence lives in `sim/attacks/` (`fv_push.py` (M) push-then-freeze, consecutive pushes, (V) `p'` veto; `refute_push.py` (c) push-then-freeze, (d) widening; `ptf_sandwich.py`; `sandwich.py` reserve-rule section), `sim/pool_model.py` (the R4 demos and `lock_fuzz`) and `sim/config_check.py` (the 1,655-run battery). **All 20 findings are resolved**: 16 FIXED as proposed or with an equivalent mechanism, R4A-8 / R4B-8 FIXED by a different mechanism, R4A-9 and R4B-10 as disclosed residuals. **Three contain proposals REJECTED with evidence**: the rent-only reserve of R4A-1 / R4B-1 (it has the same dead band once rent due exceeds `K_s`; a third rule — no liquidation reserve — is adopted instead), part of R4A-4 (two of its failing examples were artefacts of the withdrawn reserve and of position size), and the threshold vest flush of R4B-4 (it re-opens just-in-time sniping). Section references are to this document unless marked ECON; ECON changelog #74–#89 has the economic side.

| ID | Sev. | Finding | Outcome | Where the fix lives |
|---|---|---|---|---|
| R4A-1 | HIGH | v0.4's penalty reserve in `max_in` blocks honest liquidations in genuine fast pumps (the live cost sits a few tenths of a percent above the lagging reference, `max_in` collapses to `B̂_ref·(1 + ε)`, the position gaps into S1); it alone made τ = 6 and κ = 2,250 fail; ε became load-bearing | **Reproduced** (6 of the 960 grid cells, lenders −32 … −63 % instead of +0.4 … +3.1 %). **FIXED with a third rule:** of the two options offered, (1) the v0.3 rent-only reserve has the same dead band once rent due exceeds `K_s` — aged positions: C at 6 days and 90 % utilisation, S1 −46.1 % instead of liquidation +0.6 % on the new battery, 3–4 more runs at in-bound maximum rates; (2) as written, `max(V − R − π_ref − ert, min(V − R − ert, bound))` reduces to the rent-only reserve **without** the ε fallback (it never reserves π), so it is not an intermediate. Any reserve blocks exactly the buybacks in `(V − R − π_ref − ert, min(bound, V − K_s − ert)]`. v0.4.1: **a liquidation reserves nothing** (`max_in = min(⌊B̂_ref·(10⁴ + δ)/10⁴⌋, V − K_s − ert)`, SPEC §5.2); an expiry keeps rent and fee senior (no battery run changes with any expiry rule); ε enters only the expiry bound. Trade-off stated (ECON §8.2 table): held model on aged Tier A at 90 %: lenders + protocol −8.02 SOL (rent-only −1.92, v0.4 −5.72); fresh positions identical to rent-only (−2.06). The genuine-pump grid and per-second noise are in `config_check.battery()` (1,655 runs); τ and κ re-derived on it (τ = 6 loses 83 runs, κ = 2,250 38 — bounds unchanged, reasons new). ECON #50, §0 finding 5, §5.7, §12.6, §15 and §3.6 / §8.5 here ("lenders never worse off") corrected | §3.6, §3.7, §5.2, §8.5, I11; ECON §8.2, §5.7, §12.6, #74–#76 |
| R4B-1 | HIGH | Same dead band, shown with per-second price noise (±0.3 %: 42 S1 against 36; the sims set the price once per second, so `B̂_live = B̂_ref` hid it); asks for noise in the battery and a liveness invariant | **Reproduced** (42 vs 36 S1 at ±0.3 %; 6 cells worse). **FIXED** with R4A-1: no liquidation reserve, so the requested invariant holds exactly — a liquidatable, buyback-allowed position with a fresh bucket never waits while `B̂_live ≤ ⌊B̂_ref·(10⁴ + δ)/10⁴⌋` (ECON I-E11, I11 here; the exit fuzz counts `reserve_waits_liq` = 0 in 1,000 paths). Noise of ±0.3 % and ±1 % is in the battery (the lag-study pumps included) and in half of the exit fuzz's paths (22 sandwich truncations in 709 exits, all within the bound's slack). The reviewer's own remark — a fallback tolerance equal to δ is the same as reverting — is why no ε-widening was tried. ECON #50 and §0 finding 5 corrected | §3.6, I11; ECON §8.2, §5.7, §13 |
| R4A-2 | MED | An expired lock stays in `lock_norm` until its holder is touched, so every other lender's allowance counts it and early withdrawers take more than pro-rata (Σ allowances up to 3.0·T); `check()` touched everyone first and could not see it | **Reproduced** (92 of 954 sampled untouched states above `T`, worst 2.72·T). **FIXED**: a lock expiry calendar — 128 day buckets of lock terms on a zero-copy `MarketVault`, `lock_until` rounded up to its day's end, the aggregate drops a bucket when its day ends, whoever is touched (A75). Floored lock terms and aggregate fix a second violation the reviewers' lock-heavy fuzz exposed once expiry was exact (all lenders locked to rounding dust: two one-share lenders could each draw `T`; 1 of 954 states, 1.27·T). `pool_model.check()` asserts I-P4 on the state the program sees (each lender touched alone): 0 of 954. The stuck-lock scenario now pays the idle lender its pro-rata part (F 0.4, G 0.4, E 0.2 of `T`; v0.4: 0.5 / 0.5 / 0). The alternative (roll the Π epoch once `now ≥` the latest `lock_until`) was not adopted: while lenders keep locking, the latest `lock_until` keeps moving | §2.3.6, §3.3, I8, T-POT (A75); ECON §9.3 |
| R4B-3 | MED | Same (idle lender past `lock_until`, others withdraw first) | **FIXED** with R4A-2. Day buckets instead of the suggested weekly ones: a lock is extended by less than a day, not up to a week, at 2 KB of zero-copy state | as R4A-2 |
| R4A-3 | MED | FV's `p'` (penalty vs expiry fee) came from the raw "CR-liquidatable" flag; one contrary low observation fails its robust-spot leg and both box corners used the manipulated `p'` (S3 take −2.9 … −7.3 %) | **Reproduced. FIXED**: `p'` is decided at each evaluated state — the expiry fee only if expired and `CR(V_eff, B̂_FV(P, L)) ≥ L` there; a disagreement between corners makes the amount check wait. The veto case is in `fv_push.py` (V) (v0.4.1: take unchanged) and the exit fuzz's blocker runs exercise it. ECON #51, §5.6, §8.6; §3.8, §4.5, §8.5 and O15 / O19 here corrected | §3.8, §4.5, T-BOX (A65); ECON §5.6, §8.6 |
| R4B-2 | MED | Push-then-freeze defeats the box: a frozen second (honest keeper observation plus an attacker observation just beyond `P_ema`) records `obs = P_ema`, so 1 push + 3–4 frozen seconds move the medians; the same freeze holds the liquidation reference at a pushed `P_ema` | **Reproduced. FIXED** with the reviewer's candidate: `RingEntry` stores the bucket's `(lo, hi, llo, lhi)` (136 B; `Market` 3,104 B) and the box takes the lower median of the lows and the upper median of the highs; the liquidation reference is also capped at `lo_x·(1 + κ_eff)`. Results: the (M) flips fail with 0–10 frozen seconds (v0.4: +2.37 … +5.58 SOL after 4); push-then-freeze refutation delays S1 by 8 s (v0.4: no S1 within 600 s); push-then-freeze sandwich `B̂_ref` ×0.999 (v0.4 ×1.156; borrower −1.90 instead of −19.27 SOL); the flag-S3 amount waits while a freeze lasts. PTF cases are in `fv_push.py`, `refute_push.py`, `ptf_sandwich.py`, the exit fuzz (freezer runs), T-BOX and Crucible. New residual, disclosed: a contrary observation every second widens one side of the box and defers S1 or an S3 on an executable venue while paid (0.002–0.21 SOL/s at 1 %); on Raydium an LP add/remove widening moves `L` too little to matter. ECON §5.6, §12.10, §12.11 and §3.8 here no longer call R3A-1 / R3A-6 (a) closed without qualification | §2.3.4, §3.6, §3.8, §4.4, §4.5, T-BOX (A34, A65); ECON §5.6, §8.2, §12.10, §12.11 |
| R4A-4 | MED | The corner re-check covered 7 parameters; other in-bound values (ε, settlement keeper terms, λ_min, penalty 2,000) failed the battery, so "the compiled bounds are the simulated-safe region" overclaimed | **Reproduced on v0.4. FIXED**: `--scan` moves every `Params` / `TierParams` field with a liveness effect to each compiled extreme; `--targeted` re-derives the bounds on the full 1,655 runs; failing bounds tightened (settlement keeper terms ≤ their SPEC / ADOPTED values, `liq_persist_n ≤ 3`, SPEC CRs as per-tier minima); 128 core-box corners and a stress corner pass the full battery. Partly **rejected with evidence**: ε = 0 / 5 and penalty 2,000 failed only through the withdrawn reserve (0 runs now); λ_min = 10,000 (and the fee-scaled size cap) failed through position size alone — the defaults at the same `Q` give the same outcomes — so the gate now compares sizing parameters at equal size. The wording in ECON §5.7 / §15, §2.3.3 here and O18 says exactly what is covered | §2.3.3 (A73), O18; ECON §5.7, §15; `config_check.py` |
| R4A-5 | LOW | The sim reconstructed `B̂_FV` from FV with a ceiling division, so its S2/S3 lump differed from ARCH's formula by 1 lamport (76 of 86 settlements) | **FIXED**: the lump is now `x − min(x, ⌊Q·P_ema/WAD⌋ + R_due)` (R4B-6), which needs no `B̂_FV`; `fv_parts` returns FV, `B̂_FV` and `p'` for the other uses. The sim and §3.8 / §4.5 use the same formula; M0b regenerates the exit vectors | §3.8, §4.5, §9 M0b; ECON §8.6 |
| R4A-6 | LOW | M0b missed crate items: `switch_liquidity`, `risk::{s1_legs, s2_legs}`, `FeeBps::capped`, `ExitParams.liq_persist_s` and τ's argument, `waterfall::settle`'s inputs | **Confirmed against the crate (read-only). FIXED**: all five added with exact forms — one `rewrite_liquidity` with the Raydium variant (every entry `l = llo = lhi = L_ema`, `(x, y)` recomputed, bucket cleared); the single-point legs deleted; `FeeBps::capped` aligned to the sim (one combined fee at the cap, one ceiling); the field and the τ argument named; `waterfall::settle(v, kind, k_s, fv, q, p_ema, r_due)`; plus every v0.4.1 change | §2.2, §9 M0b |
| R4A-7 | LOW | After a full wipe, cashback and trader balances not yet swept went to the first depositor's shares; the wiping keeper could bundle `[sol_settle → deposit(2 units) → claim_venue_rewards]` | **Reproduced** (the dust depositor took 1 SOL). **FIXED** (the reviewer's first option): a deposit into a live wipe record requires `sweep_trader` and `claim_venue_rewards` since the wipe (`Market.flags` bits 7–8, `WipeSweepPending`; both permissionless and bundled by the depositor); the wiped lenders now get the 1 SOL. The round-3 row's "post-wipe bypass FIXED" was premature | §2.3.4, §3.3, §3.9, §3.11 (A70); ECON §9.5 |
| R4A-8 | LOW | An in-bound `lock_grace_s` (≥ 1 h) can release locks while a position stuck in `PumpMigrating` (pause + grace + S3 wait, up to 3 days) is still borrowed | **Reproduced** (lock day 30.04, position resolves day 33). **FIXED differently**: a relational check against governance values cannot protect positions that snapshot larger values earlier; instead `max_release` folds each position's own snapshot tail (`deadline + max_migration_pause_s + grad_grace_s + s3_wait_s`) into the lock horizon (lock day 34), and `lock_grace_s` covers keeper latency. ECON §9.3 states the condition | §2.3.4, §3.3, §3.4 (A57); ECON §9.3 |
| R4B-8 | LOW | Same, with `max_migration_pause_s`, `s3_wait_s` and `grad_grace_s` up to 1 day each | **FIXED** with R4A-8 | as R4A-8 |
| R4A-9 | LOW | Runs of consecutive attacker-only seconds (degraded mode) were not quantified: 5 up-pushes flip a capped S2 into S1 | **Reproduced on v0.4.1** (the new box needs 5 of 8 in either direction; 5 consecutive: +5.58 / +2.37 / +4.57 SOL for 1.09 / 1.13 / 1.96 SOL; none of 6 with one honest second between). **FIXED as a disclosed residual**: ECON §5.6 now says "5 of 8 in either direction" (the reviewer's "≥ 4 down / ≥ 5 up" described v0.4's single lower median; the bucket-extremes box of R4B-2 is symmetric), rows in ECON §12.6 and §12.11, §8.5 here and O19; keeper duties 1–2 and degraded mode are the levers | §8.5, O19; ECON §5.6, §12.6, §12.11 |
| R4A-10 | LOW | Six text inconsistencies | **FIXED**: (a) ECON #63 now states `kp + min(max(0, K_s − kp), W)`; (b) changelog row 24 says eight §10 rows; (c) `claim_sol` 8–20 k CU in both §6.1 and §6.6; (d) ECON §10.3's S3 condition says "liquidatable or expired" with the global hard cap; (e) the sim header says `settle_s2_first`; (f) R3A-8 (a) is "resolved pending O15" (round-3 table below and ECON Appendix A) | changelog, §6.6, Appendix A; ECON #63, §10.3 |
| R4B-4 | LOW | The last-lender vest flush triggers only at `shares == S`; a dust share defeats it (97 % of an honest exiter's 3 SOL of lumps to the dust holder) | **Reproduced. Partly REJECTED with evidence**: the suggested flush before any withdrawal that leaves < 1 % of `S` lets a whale that deposits 199× just before a lump take 99.5 % of it atomically — the snipe R3B-2 closed. Adopted: the disclosure (an exit forfeits the unreleased vest to whoever stays), the "last lender" rule stated as `S → 0` only, and a frontend warning | §3.3; ECON §9.4 |
| R4B-5 | LOW | Every lump restarted the 6 h window for the whole balance; 1-lamport sweeps turned the release exponential (63 % after 6 h) | **Reproduced. FIXED**: value-weighted end `vest_end' = now + ⌈(vest_bal·(vest_end − now) + x·lump_vest_s)/(vest_bal + x)⌉`; with a 1-lamport sweep every minute 98.4 % is released after 6 h (ceiling: the end never moves earlier, a griefer buys at most 1 s per transaction); T-POT case | §3.3 (A70); ECON §9.4 |
| R4B-6 | LOW | The immediate S2/S3 principal `min(x, B̂_FV + R_due)` exceeds market value by impact and fees; a token whale's `[deposit → sol_settle → withdraw → claim]` takes its share of the premium | **Reproduced** (×9 whale: 90 % of the premium). **FIXED**: only `min(x, ⌊Q·P_ema/WAD⌋ + R_due)` is immediate, the rest vests; the whale now gains 0. S1 stays immediate, with the rationale corrected (it pays `V − K_s < B̂ + ert`; vesting it would shift S1 SOL towards lenders who stay through a loss) | §3.3, §3.8, §4.5, §5.2 (A70); ECON §8.6, §9.4 |
| R4B-7 | LOW | `revalidate_venue` validated whatever slot was current at execution; `flagged_slot` was not refreshed; the curve mask omitted pAMM and each Raydium mask the other Raydium program | **Confirmed** (mainnet deploy slots equal `guard_slot` today [V9]). **FIXED**: `revalidate_venue(venue, expected_slot)` (`VenueSlotMismatch`), `flag_venue_upgraded` refreshes the recorded slot while the bit is set, masks: curve = pump + pfee + pAMM, Raydium = AMM v4 + CPMM | §2.3.2, §3.1, §3.4, T-LATCH (A7) |
| R4B-9 | LOW | A28 keeps per-market keys inline, so a v0 transaction fits 4 PumpSwap / CPMM (5 curve) cranks, not 6 | **Confirmed by the byte estimate. FIXED**: keeper batches are v1 transactions (≈ 6 shared + 6 per-market keys, up to 9 cranks); `keeper_cost.py` reports k = 4 (v0 fallback, +50 % base fees) and 6 / 9 (v1) and the byte table; U25 | §6.4, §7.1 duty 7, U25; ECON §5.8 |
| R4B-10 | LOW | A standing minimum position keeps `open_positions ≥ 1` and pins a Raydium market to a thinner eligible pool | **Confirmed. Disclosed** (the reviewer's second option): no funds at risk; remedy by an Emergency pause of opens (existing power) so the market drains, then keeper duty 12 adopts at 0 positions. The delayed adoption with positions open was not adopted: it re-creates the pending switch decision 7 removed. O18 lists it | §2.6, §7.1 duty 12, O18 |

### Review resolution — round 3 (third review, `R3A-x` / `R3B-x`)

One row per finding of the third 2026-09-27 review, plus the brief's ten simplification decisions. Each fix was checked against ECON v0.4 and its sims (the economic evidence is ECON's; ECON Appendix A lists the economic side of every row); the architecture-only rows were checked here: the per-primitive CU on SBF [V18], Agave's fixed costs [V19] and `u128` alignment per target [V20]. **All 25 findings are FIXED** (R3A-7 by removing the mechanism it concerned), except R3A-8 (a), which is **resolved pending O15** (the reviewer's alternative needs a SPEC §14 record; v0.4.1 text fix, R4A-10 f). Round 4 revised several rows (see the round-4 table): the penalty reserve of decision 1, the settlement box's medians (R3A-1, R3A-6), the lock expiry (R3B-1) and the post-wipe path (R3B-2). Two proposals inside findings are **REJECTED with evidence**: the reviewer's suggested bounds κ ≥ 1,500 and τ ≤ 8 (R3B-6), and the brief's literal "lender-adverse side for every decision" wording (decision 5). Section references are to this document unless marked ECON.

| ID | Sev. | Finding | Outcome | Where the fix lives |
|---|---|---|---|---|
| R3A-1 | MED | The FV gate protected S2 only; the kind came from instantaneous legs, so a lender-keeper's 1–2 pushed seconds flipped a capped S2 into an uncapped S1; the exit fuzz produced no S2/S3 | **FIXED**: one rule for every settlement decision, the settlement box (A65): S1 needs its EMA leg at the corner `(P_lo, L_hi)`, which one pushed second cannot reach; S2/S3 amounts must agree within `fv_dev_bps` across the box. Flip cases (1 and 2 pushed seconds) now fail (v0.3 +2.37 to +5.58 SOL); the exit fuzz produces 9 S2 and 49 S3 exits of 772 with a maximum deviation of 90 bps from the ring-median take (bound 100). §8.5's residual corrected (±1 % amount push, ≈ 13 s deferral per attacker-only second) | §3.8, §4.5, §8.1, §8.5, I12, T-BOX; ECON §5.6, §12.11, §13 (round 4: medians over bucket extremes, `p'` on the box — R4B-2, R4A-3) |
| R3A-2 | MED | Memoryful claims were not live: realised SOL could be stranded (surplus recycled only to current shares; `S == shares` divided by zero) | **FIXED**: surplus realised SOL moves to a MarketVault pot `unmatched` at its holder's touch, and `claim_sol` draws it up to the claimant's own deficit (O(1), no division, no keeper duty). The lock state moves to `MarketVault` so that `claim_sol` runs pool_model's full touch without writing `Market`. Liveness (I-P6) is part of I8 and T-POT; the reviewer's stranding test falls from 68.12 SOL to 0.00002 SOL. `S == shares` needs no special case | §2.3.6, §3.3, I8, T-POT (A56, A70); ECON §9.4, §9.6 |
| R3A-3 | MED | Contradictory differential targets (§2.2 v0.3 vs §8.1/M0 v0.2, "two-leg quotes"); undocumented sim deviations (i)–(v) | **FIXED**: M0 is stated as built against v0.3 with `B̂_str` (`B̂₂` v1.1 only); the new milestone M0b migrates the crate to v0.4 with an exact delete / change / add list; §2.2 and §8.1 name the v0.4 sims, in which ECON removed every deviation (ert on Raydium, cap reserve, CPMM fee rounding, risk-fee cap / flag / S3 on an executable venue, ring reset) and the per-tier rent index, so the differential suites run with **no tolerance or exception** (rent compared exactly) | §2.2, §8.1, §9 (M0, M0b); ECON changelog #67 |
| R3A-4 | MED | Fast-path trigger contradicted between §2.6 and §5.3, had no state, and could inherit the Emergency flag through the S3 chain; the global vs snapshot hard cap was unspecified for market-level predicates | **FIXED**: `pool_fail := !venue_executable ∨ φ_live > global hard cap`, its own chain `pool_fail_since` / `last_pool_fail_obs` (60 s gap rule) in `Market`; market-level predicates (S3 chain, `sol_settle`'s "no route now", `pool_fail`) use the global cap, risk quotes and FV the position's; A26 lists this governance reach; T-S3-BROKEN asserts the chain is identical with and without the flag | §2.3.4, §2.6, §3.0, §5.3, A26 (A71); ECON §10.3 |
| R3A-5 | LOW | Global `rent_protocol_bps` at every accrual and at the true-up: phantom or over-debited `receivable` after a governance change | **FIXED**: snapshot per position (G→P), folded into `A_j` and `credited_for`; the clamp `receivable −= min(credited_for, receivable)` stated; `receivable = 0` with no open position is part of I8 (I-P7) and fuzzed under random governance changes | §2.3.3, §2.3.5, §3.3, §4.5, I6, I8 |
| R3A-6 | LOW | (a) Refutation on the raw `P_ema` let a borrower's one-second push disarm repeatedly; (b) a spot-leg veto longer than the arm gap restarted the clock | **FIXED**: refutation and "not liquidatable" disarm only on box-robust evidence; an armed clock keeps running (`s_last = now`) on every other evaluation, including a stale bucket and a spot-leg veto. (a) S1 at most 4 s later instead of never within 600 s; (b) +0–2 s instead of +34–42 s | §3.8 (A52, A65), T-BOX; ECON §5.6, §12.10 (round 4: push-then-freeze closed by the bucket-extremes medians, R4B-2) |
| R3A-7 | LOW | The Dutch clock counted "executable" seconds in which no keeper could execute; residual attribution | **RESOLVED by removal**: the Dutch clock is gone (fixed δ = 2 %, decision 1); I11's clock clause removed; ECON re-attributes the held residual to κ opening plus the fixed 2 % | §3.6, I11 (A34; A51 superseded); ECON §8.2, §12.6 |
| R3A-8 | LOW | Non-literal defaults: (a) `p'` in FV; (b) O6 powers built before sign-off; (c) compiled bounds of unbuilt O1/O2 knobs | (a) **Resolved pending O15** (v0.4.1 wording, R4A-10 f; O15 gates M2) by the reviewer's alternative: the expiry-fee reading is kept with SPEC §4 cited ("a 2 % expiry fee instead of the full liquidation penalty") and ECON's evidence (the penalty reading moves 3–8 % of `B̂` to lenders and rewards racing S3); O15 with a recommendation. (b) **FIXED**: `set_route_broken` and `set_venue_extras` are built only after O6 is recorded as a SPEC §9 amendment. (c) **FIXED**: `fv_liq`, `settle_s2_first`, `margin_ema_own_sale` removed from `Params`, the snapshot and `validate_params`; bytes reserved | §3.1, §4.5, §9, §10 (O6, O15), §2.3.3, §2.3.5 |
| R3A-9 | LOW | Forced partial adoption vs `params_version` undefined (kink never adopted, or re-copied every prologue) | **FIXED**: the kink has its own `kink_version`, adopted only with no open position; a forced EMA adoption advances `ema_params_version` only; test "force-adopt with positions open, drain to 0, the kink is adopted" | §2.3.2, §2.3.4, §3.0 (A17, A59) |
| R3A-10 | LOW | With `S == 0` after ordinary withdrawals, later sweeps/cashback went to `unassigned`, which a dust depositor took | **FIXED**: `unassigned_sol` removed; SOL credited with `S == 0` tops up the live wipe record, otherwise goes to `protocol_fees`; the last lender to leave is paid the unreleased vest | §3.3, §3.9 (A39); ECON §9.5 |
| R3A-11 | LOW | (a) No compiled maxima for `pool_fast_route_s`, `market_max_notional`; (b) fast-path relation checked against the current, not the minimum, `s3_wait_s`; (c) δ's range depended on `ring_len`; (d) ring `L` carried over at venue changes | **FIXED**: (a) `pool_fast_route_s ≤ 420`, `pool_fast_window_s ≤ 120`, `market_max_notional ≤ 10¹⁴`; (b) `pool_fast_route_s + pool_fast_window_s ≤ 540` against the compiled `s3_wait_s` minimum; (c) moot (fixed δ); (d) every venue change rewrites the ring's `l`, `lobs`, `(x, y)` (A72) | §2.3.3, §2.5, §2.6, §4.4 |
| R3A-12 | LOW | Numerics unspecified (g, κ, median, σ, λ, D, locked SOL, EMA timing of the margin) | **FIXED**: ECON §2.1's numerics table is normative; §2.2 adds the implementation rules and the order of operations (rent before the roll; the body and the D4 margin on the post-roll cached entry); the integer forms appear where they are used (§3.6, §4.4–§4.7) | §2.2, §3.0, §3.4, §3.6, §4.4–§4.7 (A68); ECON §2.1 |
| R3A-13 | LOW | Stale texts (docstrings, ring row, allowance formula, boost wording) | **FIXED** in ECON and the sims; ARCH aligned: 104-B ring entry with `(x, y)`, allowance formula as pool_model, "130 of 130 migrations" | §2.3.4, §3.3, §4.7; ECON §2, §9.3, §17 |
| R3B-1 | HIGH | Locks decayed only below a running minimum and cleared only at `ΣB_Q = 0`, so steady demand pinned a lender's allowance at 0 indefinitely | **FIXED**: every lock expires at `lock_until = max(now, max_deadline) + lock_grace_s` (`max_deadline` monotone on `Market`); Π / `b_min` kept because it returns 94 % of the allowance within 24 h when interest fades (0 % for 31 days with `lock_until` alone); I8 asserts no lock is effective at or after `lock_until`; the reviewer's stuck-lock scenario clears (T-POT) | §2.3.4, §2.3.6, §3.3, I8 (A37, A57); ECON §9.3 (round 4: expiry calendar and the S3 tail, R4A-2, R4B-3, R4A-8) |
| R3B-2 | MED | Lump credits (penalty share, expiry fee, S2/S3 FV surplus, sweeps, cashback) sniped just in time; post-wipe bypass | **FIXED**: lumps vest linearly over `lump_vest_s` (6 h) from `Market.vest_*`, released in the prologue's accrue; settlement principal immediate; a full wipe flushes the vest into the wipe record; the last lender is paid the unreleased vest. Atomic capture 33.5 % → 0 %; a whale holding `h` hours takes `h/6` of its pro-rata lump share and none of the rent | §2.3.4, §3.3, §3.8, §5.2 (A70); ECON §9.4, §9.5 (round 4: post-wipe deposit gate R4A-7; last-lender rule only at `S → 0`, R4B-4; value-weighted vest end R4B-5; the S2/S3 premium vests R4B-6) |
| R3B-3 | MED | Naive U256 on every `mul_div` would cost ≈ 0.9–1.0 M CU per liquidation; ECON's overflow premise wrong | **FIXED with measurement**: the crate already takes the u128 path first and divides by limbs (Knuth D): 691 vs 783 CU on SBF v3 [V18], not the ≈ 8.8 k CU of a bit-by-bit divider the projection assumed. v0.4 caches the reprojection (−821 CU per ring entry per position) and drops the executable-run scan: liquidation math ≈ 35 k CU (v0.3 worst ≈ 67 k). u128 fast paths are normative, 256-bit only for ECON §2.2's products, no U256 square root; per-primitive CU budgets are an M0b CI gate (T-CU-MATH); ECON §2.2's overflow table corrected | §2.2, §6.1, §8.1, §9 M0b (A68); ECON §2.2 |
| R3B-4 | MED | `align_of::<u128>()` differs between SBF (8) and host/wasm (16): zero-copy layouts decode at wrong offsets or panic | **FIXED**: every 128-bit field of every zero-copy account is `PodU128` / `PodI128` (`[u8; 16]`, align 1) with explicit padding; compile-time size and offset asserts compiled for sbpfv3, x86_64 and wasm32 (T-LAYOUT); a lint forbids raw `u128` in `state/`; math types are never cast from account bytes. The premise was verified [V20] and `Market` recomputed (2,848 B) | §2.2, §2.3, §8.2, §8.4 (A67) |
| R3B-5 | MED | Keeper economics unpriced; throttling would silently remove the protection the attacker model assumes | **FIXED**: duties restated with batching (≈ 6 cranks per transaction), `sol_settle` / `liquidate` / `expire` counted as observations, ECON §5.8's cost model referenced; degraded mode: `CoverageGap` event, opens paused with reason `KeeperCoverage`, an optional pause-opens-only `coverage_authority`; the budget and the role are owner item O17 | §3.0, §3.1, §3.10, §7.1, §10 (A69); ECON §5.8 (round 4: batches are v1 transactions, R4B-9) |
| R3B-6 | LOW | Compiled bounds admitted values that break liquidation liveness | **FIXED**: τ is the constant `EMA_TAU_S = 5`; bounds tightened to the simulated-safe region (κ [2,500, 3,000], growth window [3, 4], persistence [2, 4], δ_liq [200, 500], clamp [5,000, 10,000], `ref_max_age_s` [1, 3], …); relational checks incl. `liq_persist_n + kappa_growth_n ≤ 7`; all 160 remaining corners pass ECON's battery; every proposal passes `config_check.py` (A73; T-CONFIG). The reviewer's suggested κ ≥ 1,500 and τ ≤ 8 are **REJECTED**: both turn Tier C/D 10×/60 s liquidations into S1 losses (ECON §5.7) | §2.3.3, §3.1, §8.2 (A73); ECON §5.7, §15 (round 4: re-derived for every liveness parameter on 1,655 runs, R4A-4) |
| R3B-7 | LOW | (a) Ring `L` at venue transitions; (b) fee tier of hypothetical-state quotes; (c) `reprice` rounding; (d) ring row | **FIXED**: (a) ring rewrite on every venue change, simulated for graduation and the Raydium fast path (T-VENUE-SWITCH); (b) hypothetical states use the live fee tier; (c) `reprice` uses `isqrt⁺`, `X` floor, `Y` ceil, so `Y/X ≥ P`; (d) ring row with `(x, y)` | §2.2, §2.5, §2.6, §4.3, §4.4 (A72); ECON §2.1, §3.1, §5.2, §10.1 |
| R3B-8 | LOW | Fast-path predicate had no state; reusing the S3 chain reintroduced V1-7 | **FIXED** with R3A-4 (`pool_fail`, its own chain) | §2.6, §5.3 (A71) |
| R3B-9 | LOW | `rent_protocol_bps` changes with positions open | **FIXED** with R3A-5 | §2.3.5, §4.5 |
| R3B-10 | LOW | Small liquidations paid the keeper less than `K_s`, so waiting for S1 paid more | **FIXED**: liquidation keeper floor at `K_s` from the borrower remainder (I-E10 in I12); small positions' dependence on the in-house keeper disclosed; owner visibility N22 | §3.6, §5.2, §7.3, I12, §10 (A74); ECON §8.3, §8.5 |
| R3B-11 | LOW | Straddle bundle fragile: tx1 named a stale `rT`; any intervening trade reverted the bundle | **FIXED**: the `complete-curve` helper reads `rT` at execution and completes the curve (no-op if complete); a separate stateless program so the core never forwards a user's signature to a venue; T-STRADDLE adds the "trade before the bundle" and "already completed" cases | §5.4, §6.2, §8.2 (A66) |
| R3B-12 | LOW | Age from listing understated the tier of long-graduated PumpSwap pools and pre-`T_CLAMP` CPMM pools | **FIXED**: O13 extended to AMM v4, PumpSwap and pre-`T_CLAMP` CPMM pools, with a recommendation; the UI's "tier and why" says so | §4.7, §7.3, §10 O13 (A12); ECON §6.2 |
| Decision 1 | — | Fixed exit bound | **ADOPTED**: `delta_liq_bps` 2 % (compiled ≥ 2 %); Dutch clock, ring `lo`, exec run, `liq_exec_since` and three parameters removed; penalty reserve added after the full attack suite showed the truncation — **withdrawn for liquidations in v0.4.1** (R4A-1, R4B-1: it blocked honest liquidations) | §3.6 (A34) |
| Decision 2 | — | EMA leg on the newest n entries | **ADOPTED**; `liq_persist_s`, `settle_persist_s` removed | §4.5 (A35) |
| Decision 3 | — | Cached reprojection per ring entry | **ADOPTED** (`RingEntry.{x, y}`) | §2.3.4, §4.4 |
| Decision 4 | — | Event-driven venue-upgrade latch | **ADOPTED** (`VenueGuard`, `flag_venue_upgraded`); per-open ProgramData, `VenueSlots`, `sync_venue_slot` removed | §2.3.2, §3.1, §6.5 (A7) |
| Decision 5 | — | No O1/O2 knobs; one ring-median consistency rule | **ADOPTED** as "the same decision on the whole box". The literal "lender-adverse side for every decision" is **REJECTED** with ECON's evidence: for refutation it is the corner that makes refutation easiest (the borrower's push would disarm), and for the amount it lets a borrower's one-second push lower FV by up to 8 % | §3.8, §4.5 (A65); ECON §5.6 |
| Decision 6 | — | Time-bounded locks | **ADOPTED** (`lock_until`); Π kept on evidence | §3.3 |
| Decision 7 | — | Raydium adoption only with no open positions; `pool_fail` fields | **ADOPTED** | §2.6 (A42, A71) |
| Decision 8 | — | u128 fast paths, U256 only where needed | **ADOPTED** and measured | §2.2, §6.1 (A68) |
| Decision 9 | — | Pod layouts and cross-target layout tests | **ADOPTED** | §2.3, T-LAYOUT (A67) |
| Decision 10 | — | Keeper batching, cost model, degraded mode | **ADOPTED**; budget is O17 | §7.1 (A69) |
| Lender SOL | — | (a) pot + lump vesting vs (b) single accumulator | **(a) ADOPTED**; (b) is owner option O16 (ECON: a 6-h holder takes 90 % of rent and lump under (b)) | §3.3, §10 |
| Reviewer simplifications | — | One settlement rule; drop O1/O2 knobs; O(1) pot; drop `lb_hist`/`ref_lo_k`; separate fast-path chain; fixed δ; newest-n EMA leg; cached reprojection; latch; S2 as one predicate; single accumulator; `lock_until`; pool selection only at 0 positions; U256 only where needed | **ADOPTED**, except the single accumulator (owner option O16) | as above |

### Earlier rounds

The round-1 and round-2 tables below are kept as written in v0.3. Several of those fixes were revised in round 3, and the round-3 table is authoritative for them: V1-1 (the Dutch clock, removed), V2-9 (the FV gate, replaced by the settlement box), V2-4 (arming, box-robust), V2-2 (recycling, replaced by the pot), V2-3 (locks, now also time-bounded), V2-12 (post-wipe credits, plus the protocol sink), V1-11 (the `VenueSlots` mirror, replaced by the latch), V2-7 (pool selection, zero-position adoption), V1-3 / V1-4 (O1/O2, not built).

### Round 1 (first review, `C0-x` / `C1-x`)

One row per finding of the first 2026-09-27 review. **All 33 are FIXED.** One sub-suggestion (C0-4 (c)) is deliberately not adopted, with the reason given in its row; no finding is rejected. Several fixes were revised in round 2; the affected rows say so and the round-2 table below is authoritative for them.

| ID | Sev. | Finding | Outcome | Where the fix lives |
|---|---|---|---|---|
| C0-1 | CRIT | Exit buybacks bounded only by a caller-controlled spot quote; penalty on measured B; rent junior; no expiry keeper reward | **FIXED**: reference-price bound (expiry: min of EMA, ring and last-second low, δ 1 %; liquidation: sustained-growth reference capped at `P_ema·(1+κ_eff)`, Dutch δ 1 → 5 %); senior rent; penalty/expiry fee on `min(B̂_live, B̂_ref)`; `K_e`; invariants and sandwich tests. Sim: expiry sandwich +43.74 → +0.35 SOL, lenders 0 | ARCH §3.6, §3.7, §5.2, §8.2–8.3 (A34, I11); ECON §8.2–8.5, §12.2, I-E1–I-E3 |
| C0-2 | HIGH | Optional remaining accounts (C3/C5/C6/P5) feed pricing; omission forces S1 or bypasses the boost gate | **FIXED**: every pricing/route/tier/guard account mandatory and key-checked; no None marker; B̂₂ defined on live and EMA legs; "omit each slot" tests; structural invariant | ARCH §3.0 (A6), §3.12, §4.3, §8.2, I14; ECON §10.2 |
| C0-3 | HIGH | Griefable post-conditions (pre-created trader WSOL ATA, lamport donations) freeze exits | **FIXED**: normalisation before every leg; post-conditions on deltas and authorities, never non-existence; ephemeral WSOL at a short.win PDA with grief-proof creation; `sweep_trader` | ARCH §2.4 (A33, A5, A46), §3.9, §8.2, I7 |
| C0-4 | HIGH | A venue interface change freezes all exits; S3 unreachable | **FIXED** (a) bounded Emergency-set trailing extras; (b) Emergency route-broken flag that only starts S3; (d) replay test of the 2026-04-28 change and "venue CPI fails" in the S3 design. (c) short-timelock adapter upgrade path **not adopted** (rationale corrected in round 2: it would shorten the users' reaction window, V1-8). *Revised in round 2: V2-1, V1-7, V1-8, V2-5, V2-6, V2-10* | ARCH §3.0 (A41), §3.1 (A40), §5.3, T-REPLAY-0428, T-S3-BROKEN; §10 O6 |
| C0-5 | HIGH | Atomic `[pump, crank, dump]` bundles steer the EMA; cost model understated | **FIXED**: per-second buckets with directional aggregation, one-second lag, later same-second observations count, ring, liquidation persistence 3/2 s, S1/S2 arming 40/30 s, robust spot, τ 5 s; atomic-bundle attacker model; Crucible action. *Revised in round 2: V1-1, V1-2, V2-4, V2-14* | ARCH §3.0, §3.8, §4.4, §4.5 (A35); ECON §5, §12.3–12.5 |
| C0-6 | MED | Share-ratio blow-up / u128 overflow bricks a per-mint market | **FIXED**: share epochs (full roll at `T + ΣB_Q = 0`, rebase 1,000ᵏ above `R_MAX`); `ShareEpoch` records; `sync_lender`; invariant | ARCH §2.3.6, §3.3, §3.8 (A39), I8; ECON §9.5, I-P3 |
| C0-7 | MED | FV cap binds in S1 | **FIXED** by owner decision D3: S1 never capped; S2/S3 FV = executable EMA cost + penalty + rent. *Revised in round 2 (V1-3): v0.2's S2 precedence and slow-EMA FV liquidity re-created the band; S1 now wins whenever its legs hold and FV is on `L_ema`* | ARCH §3.8, §4.5; ECON §8.6, I-E5 |
| C0-8 | MED | Admin can repoint venue programs; unbounded keeper/tolerance/EMA parameters; weak snapshots | **FIXED**: venue ids and PDAs compile-time constants; hard compiled bounds on every parameter; settlement, keeper, exit-bound, persistence and rate terms snapshot per position; EMA params market-level; adversarial config fuzz. *Completed in round 2: V1-5, V2-13* | ARCH §2.3.2, §2.3.3, §2.3.5, §3.1 (A8, A17, A26), §8.4, I4; ECON §2, §15 |
| C0-9 | MED | First-come-first-served withdrawals let informed lenders run before an S1 | **FIXED**: pro-rata withdrawal of unborrowed inventory with borrowed-inventory locks (SPEC §2.4 literal). *Revised in round 2 (V2-3): locks decay only on net reductions; the residual is up to ≈ 11 % depending on utilisation, not 4.3 % (O10)* | ARCH §3.3 (A37), I8; ECON §9.3 |
| C0-10 | MED | S3 depends on a guessed PumpSwap disable bitmask | **FIXED**: unknown non-zero flags block opens only and are never S3 evidence; the buy bit becomes a constant from T-DISABLE, a hard gate before M6; halts reach S3 via the route-broken flag; Raydium predicates exact from source | ARCH §5.3 (A24), §8.2, §9 |
| C0-11 | MED | Defaults contradicted owner-confirmed SPEC; docs disagreed on the FV-cap default | **FIXED**: the owner decided D1–D4 in SPEC v0.3; both documents now carry identical names, defaults and bounds (ARCH §2.3.3 = ECON §15). *Revised in round 2 (V1-4): the D3/D4 readings are now SPEC-literal by default and the alternatives are built only after owner sign-off* | ARCH §2.3.3, §9, §10; ECON §14, §15 |
| C0-12 | MED | Missing invariants and tests for the attack classes found | **FIXED**: invariants I7–I15 incl. ECON I-E1–I-E9 and I-P1–I-P5; adversarial suite and Crucible actions for every class listed | ARCH §8.2–8.4; ECON §13 |
| C0-13 | LOW | Claims pay unrealised rent; negative pending escapes via withdraw | **FIXED**: realised-only claims; withdrawal blocked while `pending < 0`; `cover_deficit`; per-lender solvency invariant. *Revised in round 2 (V2-2, V1-10): the v0.2 pro-rata claim was memoryless; claims now have memory* | ARCH §3.3 (A38), I8; ECON §9.4, I-P2, I-P5 |
| C0-14 | LOW | Close budget ignores `exec_rent_tol`; penalty base includes venue rent; keeper-chosen fee accounts | **FIXED**: close `max_in = min(max_buyback_cost, V − R − ert)`; penalty and fees on quotes; recipients validated against live venue lists; PumpSwap venue-side accounts must pre-exist | ARCH §3.5, §5.2, §3.12 (A45); ECON §8.1, §8.3 |
| C0-15 | LOW | `None` is terminal; detection reads caller-supplied optional accounts | **FIXED**: curve and canonical-pool PDAs always derived and required; `None` re-detects in `crank_price`; Raydium adoption via `propose_pool`; Raydium graduates map to Raydium venues | ARCH §2.5 (A1), §2.6, §3.2, §3.9 |
| C0-16 | LOW | S3 timer survives unobserved route recoveries | **FIXED**: continuous chain; gaps > `s3_obs_gap_s` restart it; keeper cranks no-route markets every ≤ 30 s | ARCH §5.3 (A23), §7.1; ECON §10.3, I-E9 |
| C0-17 | LOW | Every instruction write-locks `Market`; 24 M CU per-account block limit | **FIXED**: lender SOL counters on `MarketVault` so claims do not lock `Market`; keeper tight CU and priority policy; bundles; cascade benchmark. The 24 M constant is verified in Agave [V10]; other clients UNVERIFIED (U16). *Wording corrected in round 2 (V1-6): the isolation is from `Market` only; exits still write `MarketVault`* | ARCH §2.3.6 (A9), §6.6, §7.1, T-CASCADE |
| C0-18 | LOW | Stranded cashback on the trader's UVAs; creator fees amplify R1 | **FIXED**: permissionless `claim_venue_rewards` crediting lenders (the pinned IDLs show `claim_cashback` needs no user signature [V11]; the trader signs anyway); creator-fee income disclosed and indexed | ARCH §3.9 (A44), §7.2, §7.3; ECON §12.8 |
| C1-1 | HIGH | Straddle limited by the 64-entry instruction trace, not CU/locks | **FIXED**: ≥ 3-transaction Jito bundle; per-instruction trace budget; T-TRACE, T-STRADDLE. *Revised in round 2 (V1-9): priced on the executed path, keeper fallback* | ARCH §5.4 (A14), §6.2; ECON §10.2; R-PUMP §6.9 |
| C1-2 | MED | Boost gate griefable by re-creating the boost ATA with dust | **FIXED**: gate on `amount > boost_dust_lamports` and `now < graduated_at + boost_window_s`; donation test | ARCH §3.4 (A13), §8.2; ECON §10.1 |
| C1-3 | MED | Trader accounting breaks on donations | **FIXED**: normalisation and post-normalisation baselines (same fix as C0-3) | ARCH §2.4 (A33) |
| C1-4 | MED | Venue-side rent on sell legs with a 0-lamport trader | **FIXED**: trader funded with `ert` before pump-venue sells and refunded after; T-RENT extended to sells; Raydium charges no rent | ARCH §2.4 (A48), §8.2 |
| C1-5 | LOW | `solana-verify` needs `--cargo-build-sbf-args="…"` | **FIXED** | ARCH §1.1; R-TOOL Review corrections 1 |
| C1-6 | LOW | Curve round trip is 2.53 %, not 5.05 % | **FIXED** in the research note and ECON; ARCH states it where cited | ARCH §4.3; ECON §3.2; R-PUMP §4.2 |
| C1-7 | LOW | Mainnet evidence of PDA sellers not used | **FIXED**: T-PDA-USER narrowed to the buy path | ARCH §8.2, §12 U1; R-PUMP §6.1; R-VEN §3.1 |
| C1-8 | LOW | "2 levels spare" only if the venue creates no ATAs | **FIXED**: ≥ 1 in general; 2 for every leg because PumpSwap venue-side accounts must pre-exist and Raydium creates none; T-DEPTH covers the missing-ATA case | ARCH §6.3, §3.12 |
| C1-9 | LOW | `exec_rent_tolerance` 5 M below the 5,811,520 worst case | **FIXED**: `exec_rent_tol` 6,500,000 (and the pre-existence requirement) | ARCH §2.3.3; ECON §8, §15 |
| C1-10 | LOW | `bonding_curve_v2` pre-dates 2026-04-28 | **FIXED** in the research note; ARCH's legacy-fallback note corrected | ARCH §3.12; R-PUMP §3.2 |
| C1-11 | LOW | Toolchain citation pointed at the wrong ref | **FIXED** | ARCH §1.1; R-TOOL Review corrections 2 |
| C1-12 | LOW | Pyth cadence far slower than claimed | **FIXED**: `oracle_max_age_s` 120 s, keeper retries, V7 corrected (tiering only; exits never read it) | ARCH §2.3.3, §3.9, §4.7, §7.1, §13.2; ECON §6.3, §15 |
| C1-13 | LOW | Boost is proportional (20.69 %), not fixed | **FIXED** (confirmed on 130 migrations); VQR read live | ARCH §4.1, §4.7; ECON §1.2, §10.1 |
| C1-14 | LOW | Mollusk 0.15.1 declares Agave ^4.2, not 4.3 crates | **FIXED**: pin resolved Agave versions in the lockfile | ARCH §1.1; R-TOOL Review corrections 3 |
| C1-15 | LOW | 10 MiB ProgramData copied into the input region on every open | **FIXED**: added to the budget; T-CU measures with and without; mirrored-slot fallback designed (A7); cost UNVERIFIED (U5). *Revised in round 2 (V1-11): the mirror lives in `VenueSlots`, not `GlobalConfig`* | ARCH §6.5, §3.1 |

### Review resolution — round 2 (second review, `V1-x` / `V2-x`)

One row per finding of the second 2026-09-27 review. Each was re-checked before it was applied: the reviewer's probes were re-run on the v0.2 sims and reproduced (V1-1: Tier A +0.923 SOL extra, borrower 18.596 → 15.358 SOL, δ 399 bps; V1-3: B 80 %/×2.0 lenders 53.000 SOL against an executable cost of 57.304; V2-2: 100 claims take 151.49 of 153 SOL; V2-3: +23.1 % at 50 % utilisation with churn), and the live-chain facts were re-queried [V13–V16]. **26 are FIXED; V1-10 is FIXED in substance, with its literal strict form REJECTED on evidence.** Section references are to this document unless marked ECON or R-RAY; sim evidence is in `sim/pool_model.py` v0.3, `sim/shortwin_sim.py` v0.3 and `sim/attacks/{block_spot,arming,fv_push,fvcap}.py`.

| ID | Sev. | Finding | Outcome | Where the fix lives |
|---|---|---|---|---|
| V1-1 | MED | Dutch δ clock starts at the EMA-leg run, so blocking the spot leg with one low observation per second lets δ ramp while nobody can liquidate, then the attacker sandwiches at 4–5 % | **FIXED**: ring entries store the low of the bucket they integrated; δ advances only over consecutive entries on which both legs held (`liq_exec_since`); exit references use `max(L_live, L_ema, L_slow)`. Re-run: v0.2 clock Tier A +0.98 SOL over an honest keeper, borrower −3.40 SOL (4 s block); v0.3 clock δ stays 0.99 %, attack −1.34 SOL, borrower −0.28 SOL; the attack is in ECON §12, `sim/attacks/block_spot.py` and the Crucible list; ECON §0 finding 3 restated as the bound it is | §3.6, §4.4, §4.5 (A51), I11, §8.2, §8.4; ECON §0, §5.5, §8.2, §12.9 |
| V1-2 | LOW | Defensive use of the atomic primitive (freeze the EMA, fail the robust-spot leg, reset S1/S2 arming) not analysed | **FIXED**: modelled and disclosed — cost per second of a blocking observation against the borrower's equity (Tier A 0.35 SOL/s at the threshold, ≈ 4.6× at +5 %; the equity pays for 53–127 s at the threshold); arming can no longer be reset atomically (V2-4, which is stronger than "false on ≥ 2 buckets"); Crucible action "contrary observation every second" and a blocker mode in the exit fuzz; residual in O12 and §8.5 | §3.8, §4.4, §8.2, §8.4, §8.5, §10 O12; ECON §5.2, §12.1, §12.6, §12.9 (b) |
| V1-3 | MED | `fv_liq = max` plus S2 precedence cap lenders below the executable cost and cap settlements whose S1 legs all hold (C0-7 relabelled); drain scenarios ignore `λ_min`; expiry fee in FV unflagged | **FIXED**: default `fv_liq = ema` (D3 literal) and **S1 whenever all S1 legs hold** (kind chosen at execution); v0.2 readings are owner option O1 behind `fv_liq` / `settle_s2_first`; S2/S3 outcomes measured against the executable cost at EMA reserves (literal FV 1.05–1.07× it; O1 0.64–0.77×); the drain scenarios reconciled with `λ_min` (an adopted pool can lose at most half the depth positions were sized on; no drain ≤ 90 % reaches S2); expiry-fee-in-FV flagged as O15 | §2.3.3, §3.8, §4.5 (A52), I12, §10 O1, O15; ECON §0, §5.6, §8.6, §12.5 (c), §14 |
| V1-4 | LOW | Non-literal readings of binding D3/D4 built as defaults; confirmation gated only before M11 | **FIXED**: `margin_ema_own_sale = false`, `fv_liq = ema`, S1 first; every ECON table regenerated with these defaults (Tier A margin 111.9 % of `B_open` at the size cap); M2 builds only the literal behaviour and the O1/O2 alternatives are implemented only after the owner signs them off and records them in SPEC §14 | §2.3.3, §3.4, §9 (owner gates, M2), §10 O1, O2; ECON §4, §14, §15; sim `Params` |
| V1-5 | LOW | Global, non-snapshotted parameters still feed open positions' exits and settlements; A26 overstated; θ without bounds | **FIXED**: `fv_liq`, `settle_s2_first`, `fv_dev_bps`, `venue_fee_hard_cap_bps`, `ert`, `ε`, `ref_max_age_s`, `s3_wait_s`, `grad_grace_s`, `max_migration_pause_s` snapshot per position; `liq_slow_*` market-level; `S3_OBS_GAP_S` a constant; θ_A ∈ [$50 k, $50 M], θ_B ∈ [$10 k, θ_A); A26 now lists what governance can still reach (market-level EMA parameters after a forced-adoption delay, pool-selection parameters, Emergency flags) | §2.3.3, §2.3.5 (A26); ECON §15 |
| V1-6 | LOW | Moving lender counters to `MarketVault` does not isolate claims from exits | **FIXED** (wording and decision): stated that the isolation is from `Market` only; a separate claims account was considered and not adopted; the mitigation is the keeper CU/priority policy and T-CASCADE | §2.3.1, §2.3.6, §6.6; Appendix round 1 C0-17 |
| V1-7 | LOW | The no-timelock route-broken flag also triggers the Raydium fast path; A40/O6 understated | **FIXED**: the fast path triggers only on venue-side evidence (`!venue_executable` for ≥ 120 s); the flag is never an input to it; A40, §3.1 and O6 describe the flag's full effect | §2.6 rule 3, §3.1, §5.3, §10 O6 |
| V1-8 | LOW | Reader failures of non-pool venue accounts unspecified (S3 could be unreachable); frozen healthy positions; C0-4 (c) rationale inaccurate | **FIXED**: a correctly keyed venue-state account that no longer parses means "not executable", never a revert, for `crank_price` and `sol_settle` (A63); §8.5 now says the flag and S3 bound the freeze of *liquidatable* positions only, and discloses the healthy-position freeze; C0-4 (c) rationale corrected (the 48 h upgrade authority remains; a faster path would shorten users' reaction window) | §3.0, §3.1, §4.1, §5.3, §8.5 |
| V1-9 | LOW | `B̂₂` prices a straddle path the MVP does not execute; no keeper fallback | **FIXED**: straddles priced with `B̂_str` = the post-migration pool buying all of `Q` (tx3's path), live and EMA legs; `B̂₂` kept for the v1.1 in-program straddle; keeper fallback "tx1 + tx2 only, then act on PumpSwap"; sim executes the bundle path (1.9227 SOL vs v0.2's 1.8837) | §4.3, §5.4, §7.1 (A61); ECON §3.1, §10.2; sim `straddle_exec` |
| V1-10 | LOW | I8/I-P2 assert only the netted aggregate; the strict per-lender form should be asserted | **FIXED in substance; literal form REJECTED.** The literal `Σ max(pending,0) + unassigned ≤ lender_sol + receivable` does not hold in general: shortfall debits are pro-rata to current shares (the design's disclosed C0-13 behaviour), so it fails by the sum of negative pendings — the unmodified v0.2 model violates it with its own default seed (6 times, up to 333,507,609 lamports) [V17]; the reviewer's 0 violations came from seeds 1–3. What the concern needs — a negative balance never masking another lender's over-claim — is now exact: `Σ (real − claimed) + unassigned ≤ lender_sol` and `claimed ≤ real` per lender, asserted in I8, I-P2 and `pool_model.check` (60,000-op fuzzes incl. full wipes pass); the signed identity is kept separately and the entitlement excess is reported | §8.3 I8, §13.2 V17; ECON §9.4, §13; `sim/pool_model.py` `check` |
| V1-11 | LOW | A7 fallback writes `GlobalConfig` every slot | **FIXED**: mirrored slots in a separate `VenueSlots` account read only by `open_short` (A54); `GlobalConfig` has no hot-path writer | §2.3.1, §2.3.2, §3.1, §6.5 |
| V1-12 | LOW | Minor ARCH/ECON inconsistencies (a)–(f) | **FIXED**: (a) `cr_open_bps` scope T in both; (b) `ref_lo_k` removed (the one stored completed bucket; constant); (c) G28 restored in ECON §14; (d) `AGE_C_S`, `AGE_A_S` in both registries as SPEC constants; (e) ECON boost wording now "130 of 130 migrations"; (f) 485,764,634 (the program's charge; the quote rounds to …635) | §2.3.3, §4.7; ECON §1.2, §3.2, §14, §15, changelog #19 |
| V1-13 | LOW | Curve and PumpSwap sell legs define `S` and the refund inconsistently | **FIXED**: one post-funding baseline; `S = trader lamports + trader_wsol − ert` (net of venue rent) on both pump venues; the borrower is refunded exactly `ert`; the venue's slippage argument applies to the gross credit and short.win's `S ≥ min_out` to the net `S`; PumpSwap `r` reported | §2.4 rule 3, §2.4.1, §3.10 |
| V2-1 | HIGH | `route_ok` (incl. the Emergency flag and the fee hard cap) drives both S3 evidence and exit predicates: the flag pauses closes / liquidates on the EMA leg alone / arms the fast path; a Raydium 5.01 % fee does the same | **FIXED**: split into `venue_executable` (quotes, closes, liquidations, expiries, "spot leg true", observation, fast path) and `s3_no_route` (S3 chain and opens only); every risk quote caps the fee at the hard cap; T-S3-BROKEN asserts the flagged run's liquidatable set and closes equal the unflagged run; verified CPMM `update_amm_config` accepts any `t + c < 10⁶` [V13] | §3.1, §3.4–§3.8, §4.3, §4.5, §5.1, §5.3 (A50), I10, I16, §8.2 |
| V2-2 | MED | Realised-only claim `pending·sol/(sol + receivable)` is memoryless; repeated claims drain other lenders' realised SOL | **FIXED**: realised SOL allocated to shares on arrival (`acc_r`, per-lender `real`, `claimed`); `claim = min(pending, real − claimed, lender_sol)`; surplus recycling for just-in-time depositors; fuzz action `claim_sol ×N` (the repeat pays 0) and invariant `Σ claimed ≤ realised allocation`. Re-run: 100 claims take 76.5 SOL (fair share), not 151.5 | §2.3.4, §2.3.6, §3.3 (A56), I8; ECON §9.4, §9.6, §13 |
| V2-3 | MED | Locks decay on gross resolutions (churn unlocks a runner); residual stated only for 90 % utilisation | **FIXED**: Π moves only when ΣB_Q falls below its running minimum since the first lock (A57); fuzz ghost "churn never reduces a lock"; residual restated by utilisation (+4.3 % at 90 %, +7.4 % at 80 %, +10.7 % at 60 %, +11.1 % at 50 %, +9.5 % at 30 %; churn now adds nothing) and disclosed; the withdrawal queue is the exact fix (O10, "reconsider") | §3.3, §7.3, §10 O10; ECON §0, §9.3, §9.6 |
| V2-4 | MED | Arming reset by evidence the caller controls in the same transaction; sim/ARCH mismatch | **FIXED**: one eligibility clock for S1 and S2; disarm only when the EMA leg fails or the EMA state refutes both (`B̂_ema + ert ≤ V − K_s ∧ σ(min(L_live, L_ema)) ≤ σ_max`); a failing spot leg and a stale bucket change nothing (sim aligned); kind chosen at execution. Re-run: v0.2 — a borrower's dump around `sol_settle` every 20 s blocks S1 for > 600 s in every market; v0.3 — settlement time unchanged | §3.8 (A52), §5.1, §7.1, I12; ECON §5.6, §12.10; `sim/attacks/arming.py` |
| V2-5 | MED | No-route liveness: healthy positions frozen until their deadline; halted pools re-weight `P_ema` | **FIXED** (disclosure and design; shorter bounds are owner options): no observations while the venue cannot execute (A64), `SolSettle.ema_age_s`; freeze bound stated in §5.3, §8.5 and the frontend disclosures; owner options (a) exit-only fast path to any verified pool, (b) borrower-initiated SOL close (SPEC amendment), (c) N-hour no-route ⇒ liquidatable (SPEC amendment) in O14; Fartcoin example re-verified [V15] | §3.0, §4.4, §5.3, §7.3, §8.5, §10 O14; ECON §5.1, §10.3; R-RAY §12.4 |
| V2-6 | MED | Extras are global fixed keys: cannot express per-mint/per-pool PDAs; one hostile Emergency write could brick exits | **FIXED**: extras entries are fixed keys or derivation templates (constants + `{mint, pool, trader, bonding_curve, coin_creator}`), re-derived on-chain; two Emergency sets (current, previous) plus `none`, chosen by the caller, in `VenueExtras` (A41, A55); T-REPLAY-PDA; U24 | §2.3.2, §3.0, §3.1, §8.2, §12 |
| V2-7 | LOW | Fast path adopts the first candidate; listing-window rules undefined after a fast path; "`D` cannot be pushed by trading" is wrong | **FIXED**: best candidate within a 60 s collection window; 5 % listing-window margin; adoptions inside a window do not extend it; wording "`D` can be pushed only at fee cost" with the cost formula; the fast path excludes the flag (V2-1) | §2.3.3, §2.6 (A60), §7.1; R-RAY §12.2 |
| V2-8 | LOW | Opens gated on `λ_now`, which unlocked deposits can lower (grief) | **FIXED**: no `λ_now` open gate; tier and cap liquidity bounded by locked depth / `λ_min` (`R_eff = min(R_now, R_slow, D·√P_ema/λ_min)`); `λ_min` kept for eligibility and adoption | §2.6 rule 5, §3.4, §4.6, §4.7 (A60), I15; ECON §6.2; R-RAY §12.6 |
| V2-9 | LOW | FV reads the instantaneous `P_ema`, movable ±16.7 % by one attacker-only second | **FIXED**: S2/S3 execute only while `P_ema`, `L_ema` are within `fv_dev_bps` (1 %) of the medians of the ring's integrated observations (A53); FV stays literal. Re-run on the drained RA example: lender push +5.58 SOL without the gate, +0.41 SOL with it (a median of EMA values would not do: +3.13 SOL, because the push's decaying bump lifts it); push-then-settle cases in ECON §12.11 | §3.8, §4.5, I12; ECON §8.6, §12.11; `sim/attacks/fv_push.py` |
| V2-10 | LOW | Exact equality on delivered tokens makes any venue over-delivery freeze exits | **FIXED**: buy legs require `≥ Q`; the surplus stays in `pool_vault` as a donation and is emitted (A62) | §2.4 rule 3, §2.4.1, §3.5, §3.6 |
| V2-11 | LOW | λ counts locker-held LP as circulating: locker-launched pools are list-only | **FIXED** (recorded): coverage gap in §2.6 and O4 with a live survey (67 of 311 creator-fee CPMM pools with ≥ 25 SOL have λ < 50 %) [V16]; v1.1 option to count key-checked permanent-locker vaults | §2.6, §10 O4; R-RAY §12.3 |
| V2-12 | LOW | Lenders walk every epoch after a wipe; post-wipe credits go to the first (dust) depositor | **FIXED**: a lender converted to 0 shares jumps to the current epoch; SOL credited while `S = 0` after a full wipe tops up the wipe record for the old lenders until the next deposit (A58). Re-run: 1 SOL swept after a wipe goes to the old lender, 0 to a dust depositor; a lender 5 wipes behind catches up in one touch | §2.3.1, §2.3.4, §2.3.6, §3.3; ECON §9.5, §9.6 |
| V2-13 | LOW | A perpetual minimum position pins stale EMA parameters on a market | **FIXED**: a new EMA-parameter version reaches markets with open positions `ema_force_delay_s` (default 7 days) after it is applied (A59); `util_kink_bps` excluded (rent is locked); the delay bound is disclosed in A26 | §2.3.2, §2.3.3, §2.3.5, §3.0 |
| V2-14 | LOW | The directional freeze works for defenders too; L observations movable at zero fee on Raydium | **FIXED**: symmetric freeze documented and priced; Crucible action "contrary observation every second"; exit references use `max(L_live, L_ema, L_slow)`. Flooring L observations at locked depth is not added: `√(X·Y) ≥ D` holds by construction, so the push is already bounded at `1 − λ ≤ 50 %` | §3.6, §4.4, §8.4; ECON §5.2, §8.2, §12.9 |

## Appendix B. Earlier changelogs

Section numbers in these tables refer to the document version current at the time.

### Changelog v0.2 → v0.3 (second review round)

| # | Change | Issue | Where |
|---|---|---|---|
| 1 | **SPEC-literal defaults for the D3/D4 readings.** The margin's EMA leg is quoted on EMA reserves as they stand (`margin_ema_own_sale = false`); FV uses the fast liquidity EMA (`fv_liq = ema`); **S1 takes precedence whenever all its legs hold**, so S1 is never capped. The v0.2 readings stay available as owner options O1/O2, and M2 builds only the literal behaviour until the owner signs them off. | V1-3, V1-4 | §2.3.3, §3.4, §3.8, §4.5, §9, §10 |
| 2 | **One predicate split in two.** `venue_executable` (the venue's own state) drives quotes, closes, liquidations, expiries and the "no route ⇒ spot leg true" rule; `s3_no_route` (= not executable, or the route-broken flag, or the fee above the hard cap) only feeds the S3 chain and opens. Every risk quote caps the fee at `venue_fee_hard_cap_bps`. The Raydium fast path triggers on venue-side evidence only. | V2-1, V1-7 | §3.1, §4.3, §4.5, §5.3, §2.6 |
| 3 | **Dutch exit clock counts executable seconds only.** Ring entries store the low of the bucket they integrated; δ grows only over consecutive entries on which both legs held, so blocking the spot leg with one low observation per second restarts it. The defensive direction (a borrower vetoing its own liquidation) is modelled and disclosed. | V1-1, V1-2, V2-14 | §3.6, §4.4, §4.5; ECON §8.2, §12.9 |
| 4 | **Settlement arming that only non-atomic evidence can reset.** One eligibility clock for S1 and S2; disarm only when the EMA leg fails or the EMA state refutes both S1 and S2; "not liquidatable" on the spot leg alone and a stale bucket change nothing; the kind is chosen at execution. | V2-4, V1-2 | §3.8; ECON §5.6, §12.10 |
| 5 | **FV gate.** S2/S3 settle only while `P_ema` and `L_ema` are within `fv_dev_bps` (1 %) of the median of the ring's integrated observations; exit references use `max(L_live, L_ema, L_slow)`. | V2-9, V2-14 | §3.6, §3.8, §4.5; ECON §12.11 |
| 6 | **Liveness without a route, stated honestly.** No observations are recorded while the venue cannot execute; the freeze bound for healthy positions (up to their deadline) is disclosed; owner options for a shorter bound (O14). Reader failures of correctly keyed venue accounts mean "not executable", never a revert, for `crank_price` and `sol_settle`. | V2-5, V1-8 | §3.0, §4.1, §5.3, §8.5, §10 |
| 7 | **Venue extras as derivation templates** (constants and `{mint, pool, trader, bonding_curve, coin_creator}` placeholders), two Emergency-set sets plus "none", selectable by the caller, in a separate `VenueExtras` account. | V2-6 | §2.3, §3.0, §3.1 |
| 8 | **Lender pool.** Claims pay only realised SOL allocated to the lender (memoryful; repeated claims pay 0); locks decay only on net reductions of the borrowed inventory (running minimum); a lender with 0 shares skips later epochs; SOL credited after a full wipe tops up the wipe record for the old lenders. The strict per-lender invariant is asserted on the claim side; the literal strict entitlement form is shown not to hold. | V2-2, V2-3, V2-12, V1-10 | §2.3.6, §3.3, §8.3; ECON §9 |
| 9 | **Raydium pool selection hardening.** Fast path adopts the best candidate seen during a 60 s collection window; 5 % margin in the listing window; adoptions inside a window do not extend it; opens are sized on locked depth instead of being refused on `λ_now` (no more open-blocking by unlocked deposits); the locker coverage gap is recorded. | V2-7, V2-8, V2-11 | §2.6, §3.4, §4.6, §4.7, §10 |
| 10 | **Snapshots and bounds completed.** `fv_liq`, S2-precedence option, `fv_dev_bps`, `venue_fee_hard_cap_bps`, `ert`, `ε`, `ref_max_age_s` and the S3 timing terms are snapshot per position; `liq_slow_*` become market-level; `ref_lo_k` and `s3_obs_gap_s` become constants; θ_A/θ_B get compiled bounds; A26 now lists what governance can still affect. EMA parameters can be force-adopted on markets with open positions after a 7-day delay. | V1-5, V2-13, V1-12 | §2.3.3, §2.3.5, §3.0 |
| 11 | Straddles priced on the executed path (the position buys all of `Q` on the post-migration pool) and a keeper fallback; one sell-leg baseline for both pump venues; buy legs accept over-delivery (surplus to the pool); mirrored deploy slots in a `VenueSlots` account; `MarketVault` contention stated as it is. | V1-9, V1-13, V2-10, V1-11, V1-6 | §2.4, §3.1, §5.4, §6.5, §6.6 |
| 12 | Tests, invariants and Crucible actions for every second-round attack; Appendix A round-2 table. | all | §8, Appendix A |

### Changelog v0.1 → v0.2

| # | Change | Issue / decision | Where |
|---|---|---|---|
| 1 | **Raydium AMM v4 and CPMM are MVP venues.** New venue kinds `RaydiumV4`, `RaydiumCpmm`; readers, exact math, adapters, route predicates, admin exposure, budgets. Raydium-graduated pump coins now map to a Raydium venue instead of `None`. `None` means "list-only" and is never terminal. | **D2**, C0-15 | §2.5, §3.12, §4, §5.5, §6 |
| 2 | **Raydium pool selection** by *locked constant-product depth* `D = λ·√(X·Y)`, which trades cannot move; listing window, hysteresis switch with delay, fast path on incumbent route failure. | D2 | §2.6 |
| 3 | Owner decisions applied: direct PumpSwap (D1), Raydium (D2), S1 never capped and S2/S3 capped at executable fair value (D3), opening CR enforced at spot and EMA (D4). Their open-decision entries are removed. | **D1–D4**, C0-7, C0-11 | §3.4, §3.8, §10 |
| 4 | Adopted defaults Q4–Q24 applied as PROPOSED parameters with ECON §15 names, defaults and compiled hard bounds. | brief | §2.3.3, §11 |
| 5 | **Exit buybacks bounded by a manipulation-resistant reference price** (ECON §8.2), senior rent, penalty and expiry fee on quotes, expiry keeper reward. | C0-1, C0-14 | §3.6, §3.7, §5.2 |
| 6 | **Every account that feeds a pricing, route, tier or guard decision is mandatory and key-checked.** The "None marker" for optional slots is gone. B̂₂ is defined on the EMA leg too. | C0-2, C0-15 | §3.0, §3.12, §4.3 |
| 7 | **Trader-state normalisation** at the start of every venue leg; post-conditions assert deltas, never non-existence; ephemeral grief-proof WSOL account at a short.win PDA address; Raydium sells use a delegate on the pool vault, buys deliver straight into it. | C0-3, C1-3 | §2.4 |
| 8 | **Venue-interface failures reach S3:** bounded, Emergency-set trailing extra accounts per venue instruction; Emergency "route broken" flag that only starts the S3 timer; replay test of the 2026-04-28 pump account change. | C0-4 | §3.0, §3.1, §5.3 |
| 9 | **Observation buckets, EMA ring and persistence** (ECON §5): τ = 5 s, directional per-second aggregation, one-second lag, liquidation persistence 3 updates / 2 s, S1/S2 arming 40 updates / 30 s, per-position arming state. | C0-5 | §4.4, §4.5, §3.8 |
| 10 | Venue program IDs and derived PDAs are **compile-time constants**; **hard compiled bounds** on every parameter; settlement, keeper, exit-bound, persistence and rate terms **snapshot per position**; EMA parameters change per market only when it has no open position. | C0-8, Q9, Q23 | §2.3, §3.1 |
| 11 | Lender pool: **share epochs** with `ShareEpoch` records, **pro-rata withdrawal** with borrowed-inventory locks, **realised-only claims** and `cover_deficit`. | C0-6, C0-9, C0-13 | §3.3 |
| 12 | Rent: three **basis indices** per market replace the four per-tier indices, so each position's rate curve is locked exactly (ECON §7 edited to match). | Q9, C0-8 | §4.5 |
| 13 | PumpSwap `disable_flags`: unknown non-zero flags block opens only; S3 evidence comes from a resolved bit (hard test gate T-DISABLE) or the Emergency flag. Raydium disable predicates are exact from source. | C0-10 | §5.3 |
| 14 | Instruction-trace budget (64 entries) per instruction; straddle is a Jito bundle of ≥ 3 transactions; stack-depth headroom corrected; ProgramData input-region copy added to the CU budget with a mirrored-slot fallback. | C1-1, C1-8, C1-15 | §5.4, §6 |
| 15 | Venue-side rent: `exec_rent_tol` 6.5 M lamports on pump venues (0 on Raydium); traders are funded before **sell** legs too; fee recipients are validated against live venue lists and their token accounts must already exist. | C1-4, C1-9, C0-14 | §2.4, §3.12 |
| 16 | Boost-window gate uses a dust threshold and a time window. | C1-2, Q15 | §3.4 |
| 17 | Continuous S3 no-route chain (observation gaps > 60 s restart it). | C0-16 | §5.3 |
| 18 | `claim_sol` no longer write-locks `Market`; lender SOL counters live on `MarketVault`; keeper CU and priority-fee policy; cascade benchmark. | C0-17 | §2.3.6, §7.1 |
| 19 | Stranded venue cashback is claimable (permissionless, to lenders); creator-fee income from short.win volume is disclosed (R1). | C0-18 | §3.9, §7.3 |
| 20 | Invariants and adversarial suite extended (ECON I-E1…I-E9, I-P1…I-P5, plus architecture invariants). | C0-12 | §8 |
| 21 | Milestones re-planned: M0 math and M1 core program are venue-free; M2 builds the whole position engine against a test-only mock venue; Raydium is its own milestone (M3) before pump venues. | brief | §9 |
| 22 | Fact corrections: verifiable-build syntax (C1-5), toolchain citation (C1-11), Mollusk Agave version (C1-14), Pyth cadence and `oracle_max_age_s` 120 s (C1-12), proportional boost (C1-13), PDA-user evidence narrowed to the buy path (C1-7), pump remaining-account history (C1-10), round-trip cost (C1-6). | C1-5…C1-14 | §1.1, §4.7, §8.2 |
| 23 | Decision register renumbered `A1…A49` (v0.1 `D1…D33` clashed with owner decisions D1–D4); v0.1 → v0.2 mapping in §11. | — | §11 |
