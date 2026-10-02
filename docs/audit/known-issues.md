# short.win known issues register

This is the one list of what short.win knowingly allows. Auditors should not report these as
findings unless the stated bound is wrong or can be exceeded. Each row gives:

- **Bound**: the numeric limit, as measured or simulated in the cited section.
- **Decision**: the owner decision or open owner item that covers the row.
- **Status**:
  - **accepted**: the owner recorded it in SPEC §14 or ARCH §10, with the date;
  - **disclosed**: built and disclosed, but the owner has not yet accepted it (the item is
    named);
  - **open**: the owner has to choose, and the build default is in force until then;
  - **by design**.

The register merges these sources:

- ARCH §10 O19 (the disclosed residuals) and the M7 owner items;
- SPEC §14;
- the residuals in ARCH §2.6, §3.x, §6.6 and §8.5, and those in the changelog rows;
- ECON §8.2, §9.3–§9.5, §12.2–§12.12 and §17.

The numbers are copied from those sections at commit `e857a2a`. They are not re-derived. When
a design doc changes, its row here changes in the same commit (docs/AUDIT.md §12).

Abbreviations:

- `G`: the global daily keeper budget.
- `V`: a market's value at risk to its lenders.
- `D`: a pool's locked depth.
- `B̂`: the executable buyback cost.
- `q`, `Q`: the borrowed tokens (one position / the whole market).
- `x`: the pool's token reserve.
- Tier A–D: the SPEC §4 risk tiers.
- "the sim": `docs/design/sim`.
- "the fork": the LiteSVM tests on dumped mainnet programs.

## 1. Economic residuals (owner item O19 unless stated)

| ID | Statement | Bound | Decision | Status |
|---|---|---|---|---|
| KI-01 | Exit sandwich under the fixed 2 % bound: a keeper that sandwiches a liquidation takes up to ≈ 2 % of the reference cost from the borrower, at the first second or later (ECON §12.2) | Capped Tier A position: keeper +1.50 SOL over an honest keeper, borrower −2.02 SOL; on an expiry +0.35 SOL; lenders lose nothing | O19 | disclosed |
| KI-02 | Held model: an attacker holds the pool at a manipulated price for whole seconds. A keeper can then take the remainder of a borrower who is already being liquidated (ECON §12.6) | Tier A: keeper +5.66 SOL over an honest keeper, borrower −19.5 SOL | O19 | disclosed |
| KI-03 | A liquidation reserves nothing (v0.4.1). Lenders and protocol lose the penalty, and on aged positions the rent, that the bought-down vault no longer covers (ECON §8.2) | Tier A −2.06 SOL fresh; −8.02 SOL at 25 days and 90 % utilisation. The alternative, a rent-only reserve, costs one extra S1 in the 1,655-run battery | O19 (accept, or the rent-only reserve: a program change) | disclosed |
| KI-04 | OI-cap baskets on low-fee Raydium pools stay marginally profitable for an attacker (ECON §12.5–§12.6) | +0.5 to +8.5 SOL at the OI cap | O19 | disclosed |
| KI-05 | Paid veto: a borrower can keep failing the robust-spot leg every second and so delay its own liquidation (ECON §12.9 (b)) | Limited by the borrower's own equity. At ×1.00: 56 / 68 / 95 s on the A / B / C example markets, at 0.35 / 0.19 / 0.023 SOL per second | O19 | disclosed |
| KI-06 | A one-second push can move the amount of an S2 or S3 settlement (ARCH §3.8; ECON §12.11) | At most `fv_dev_bps` (±1 %). The Emergency-flag S3 case moves +0.70 SOL | O19 | disclosed |
| KI-07 | Settlement deferral: each attacker-only second defers a settlement by ≈ 13 s. A paid contrary observation every second defers S1, or an S3 on an executable venue, for as long as it is paid (ECON §5.6, §12.10, §12.11) | 0.002 SOL/s (the RC example market) to 0.21 SOL/s (A) at a 1 % round trip | O19 | disclosed |
| KI-08 | In degraded mode (no keeper, or a censored keeper), 5 consecutive attacker-only seconds move the settlement box's medians and flip a capped S2 into an uncapped S1 (ECON §12.11) | +2.37 to +5.58 SOL for the attacker on the drained RA model cases, at a cost of 1.1–2.0 SOL | O19 | disclosed |
| KI-09 | A lender who exits forfeits its unreleased vest to the lenders who stay (ECON §9.4) | The 6 h vest of lumps and premiums | O19 | disclosed |
| KI-10 | SOL that arrives while a market has no lender goes to the protocol | Per arrival | O19 | disclosed |
| KI-11 | Rebase-orphan dust (ECON §9.5) | Negligible except in dust-sized pools | O19 | disclosed |
| KI-12 | Sale-wrapped open (M7-SEC-4): the open records `N_open` on the reserves after the open, which a sale in the same transaction lowers. `N_open` sets the launch cap `market_max_notional`, `min_notional` and the open fee. The D4 margin is not affected (ARCH §3.4). On the curve the same holds for the open fee and the notional limits (ARCH §4.6 "Residual (not changed)") | `N_open` ≥ 0.90 of the honest quote (measured 0.9096). The launch cap can be exceeded by at most ≈ 11 % (measured +8.0 % at 25 SOL). `min_notional` can be undercut by ≈ 9 % | SPEC §14 M7-11 (O19) | accepted (owner, 2026-10-01) |
| KI-13 | Run residual: lenders who withdraw just before an S1 gain over those who stay (ECON §9.3) | The runner gains +4.3 % over fair value at 90 % utilisation and up to +11.1 % at 50 % | O10 (standing: accepted for the MVP; a withdrawal queue in v1.1) | disclosed (standing item) |
| KI-14 | Gap risk stays with the lenders. A gap that outruns the whole cushion is a lender loss (SPEC R3; ECON §11.5). Gap frequencies are not calibrated (U21) | Losses start beyond ≈ 2.1× / 2.6× / 3.1× / 4× (Tier A / B / C / D). At 10× lenders lose 78 / 73 / 68 / 58 % of the lent tokens' value | SPEC D3/D4; O9 (calibration, M10 WP-9) | by design; frequencies TODO(WP-9) |
| KI-15 | Creator self-dealing (SPEC R1). The coin creator earns venue creator fees on every short.win leg (ECON §12.8). Per-wallet caps are per key, so one actor with four wallets can fill a market's OI cap (M7-SEC-9) | The per-market OI and notional caps | SPEC §12 R1 | disclosed |

## 2. Pool switches, third-party liquidity and graduation (owner item O18 unless stated)

| ID | Statement | Bound | Decision | Status |
|---|---|---|---|---|
| KI-16 | A standing minimum position can pin a Raydium market to a thinner pool (ARCH §2.6, R4B-10) | Until the position closes. The remedy is an Emergency pause of opens | O18 | disclosed |
| KI-17 | M4 rule residuals (ARCH §2.6 "Residuals of the M4 rules" (a), (b)): (a) a list-only market without a price judges the floor at the candidate's spot, which one transaction can push; (b) a fast-path candidate's spot is compared with `P_ema` only through the window floor at `min(P_ema, P_cand)` | (a) Bounded by the price warm-up (M4V-5). (b) An attacker's candidate at ≈ 29 % of `pool_d`, held at 3×, is not liquidatable before the warm-up gate (M4R) | O18 | disclosed |
| KI-18 | Warm-up in degraded mode (ECON §12.12 residual 1). If all 8 warm-up buckets are attacker-only, the attacker seeds `L_ema = D`; on pools with λ < 50 %, 4 such buckets suffice | A position within ≈ 2.8× of the carried `P_ema` can be liquidated at the gate at the pumped price. This is what the same attacker-only seconds force in steady state | O18 | disclosed |
| KI-19 | Capital deposit freeze (residual 2): deposit-wrapped cranks every second hold `L_ema` | The EMA leg is delayed by at most ≈ `(1 − H/L)·q/x` in price. The reference stays at the honest lows while the guard is set and the keeper observes every second | O18 | disclosed |
| KI-20 | Withdraw contest while the guard is set (residual 3) | The borrower overpays by at most ≈ `(1/λ − 1)·q/x` of `B`: ≈ 0 on λ ≈ 1 pools, +4.04 % (1.24 SOL) in the sim | O18 | disclosed |
| KI-21 | M4R-2 is closed only for honest dumps before the window opens (residual 4) | One pump-wrapped window-opening proposal (0.03–0.04 SOL) restores M4's `P_ema` judgment, which is never looser | O18 | disclosed |
| KI-22 | Price-seed bounds after a graduation (residuals 6, 17; M6-SEC-1). The seed falls by at most one v0.4.1 step per integration, counted from the running seed | Delay ≤ ≈ 4.5 s from a seed at the pool's price. After a graduation that jumped the price: +28 s instead of +12 s (D@80 %), a 0.63–1.08 SOL round trip; the lenders are whole | SPEC §14 M6-SEC-1 | accepted (owner, 2026-09-30) |
| KI-23 | FITX-1: withdraw-wrapped cranks freeze a genuinely reached low (ECON §12.12 residual 10) | It forces only the liquidation that the same LP gets by withdrawing for good: same second, same price, the borrower no worse off | SPEC §14 FITX-1 | accepted (owner, 2026-09-29) |
| KI-24 | A pool switch carries positions only if every one fits; otherwise the positions settle S3 at fair value at their deadline (#99, the mark FIT-3; residuals 12, 14). Healthy positions stay frozen at the pre-halt `P_ema` meanwhile | Lenders +6.9–7.1 % over `q` at the frozen price, but −46 % against a 2× move during the halt. The mark lasts at most the tier term (30 / 14 / 7 d at A / B / C) | SPEC §14 #99, M4F-1 | accepted (owner, 2026-09-29) |
| KI-25 | Where the fit is judged (FIT-1, residual 13). After a genuine k× pump during the halt, a carried position can be larger than a native open's cap on the adopted pool | Up to `λ·√k` × the native cap (the fork: 1.085 / 1.109 / 1.366 at k 1.2 / 1.5 / 1.9). It is liquidated within 1 s of the control | O18 | disclosed |
| KI-26 | ECON residual 16 (M6D, margin part). A PumpSwap position opened while a third-party deposit is held posts its margin on the deposited depth | At Tier C's V/B edge, a pulled deposit turns a one-second 3× gap's liquidation into an S1, with the lenders whole. The gap class loses 0.3–3.5 more points of `Q` | O18 (accept, or price the open's margin on the burned depth: **a program change**) | open |
| KI-27 | PumpSwap is sized on its burned LP only (M6D-1). Pools with genuine third-party LP get less capacity | e/acc 0.93×; −6.13 % capacity after 2 h (`capacity_on_the_canonical_pool_fixtures`) | SPEC §14 M6D-1 | accepted (owner, 2026-09-30) |
| KI-28 | Exit delay after graduation. The keeper's tx3 records the new pool's first bucket (OWNER-M6SK-1) | Exits land 8–12 s after the migration (M6R-4). Tracking of a genuine ×3 pump slows from +9 s to +16 s (M6SK-1) | SPEC §14 M6R-4; OWNER-M6SK-1 (keeper only) | M6R-4 accepted (owner, 2026-09-30); M6SK-1 open |
| KI-29 | A 211-B PumpSwap pool at minimum rent takes 304,800 lamports from the first sale (OWNER-M6R-5) | An open there needs `S_exp` ≥ 0.3048 SOL at `exec_tol_bps` 10 (≈ 1.7 % of `X` on the 19.13-SOL legacy pool) | OWNER-M6R-5 (O18) | accepted |

## 3. Keeper coverage, budget and operations (owner item O17 and the M7 items)

| ID | Statement | Bound | Decision | Status |
|---|---|---|---|---|
| KI-30 | Degraded mode: a market the keeper no longer covers has its opens paused, and its exposure is the held-model bound (ARCH §8.5; ECON §5.8) | The KI-02, KI-03 and KI-08 bounds | SPEC §14 O17, M7-13/-14 | accepted (owner, 2026-09-30 / 2026-10-01) |
| KI-31 | OWNER-M7-2: LP-wrapped liquidity-anomaly triggers spend the free pool. After that, each market keeps only its entitlement | ≈ 0.1–0.7 SOL a day of the attacker's money. With 20 covered markets and `G` = 1 SOL, the first market takes 22,720 triggers unbatched / 136,320 batched | OWNER-M7-2 ((a) accept, or (d) an hourly cap: keeper only) | open |
| KI-32 | OWNER-M7-6: exit-critical cranks are never throttled, so spending beyond `G` is an alerted overdraft | Worst case is junk above the floor held near a threshold: 14 / 3 junk markets (0 / 100,000 µlamports/CU) spend `G` = 1 SOL a day. At `V` 1 SOL this takes 40 / 8.5 SOL of capital and 0.076 / 0.016 SOL a day | OWNER-M7-6 ((a) as built, or (b) cap the overdraft: keeper only) | open |
| KI-33 | OWNER-M7-17 (3): bursts on any covered market can drain the free pool | Junk that hovers and bursts degrades every market ranked below it with 3 / 2 junk markets. At `V` 1 SOL that needs 8.5 / 5.7 SOL of capital and 0.61 / 0.41 SOL a day | OWNER-M7-17 ((a) accept, or (b) confine bursts: keeper only) | open |
| KI-34 | OWNER-M7-17 (4): honest markets below `var_floor_lamports` (1 SOL of `V`) get only the 30-s idle cadence | ECON §11.3's 30-s rows: Tier C / D 10×/60 s pumps end in S1 (lenders −67.7 % / −58.3 %), where per-second coverage liquidates them (+0.9 % / +1.7 %) | OWNER-M7-17 (confirm the defaults) | open |
| KI-35 | OWNER-M7-3 residual: a hijacked coverage key can set the pause again after each clear, until the timelocked rotation | Rotation takes ≥ 86,400 s. Each set costs the attacker one transaction, and one Emergency clear (≈ 8,000 CU) undoes it. Opens resume 5 s after a clear | SPEC §14 M7-3 | accepted (owner, 2026-10-01) |
| KI-36 | OWNER-M7-8: crank batches also write-lock the keeper's fee payer, but `payer_cu_per_slot` (20 M) counts only exits | 200 markets ≈ 5.5 M CU a slot on the mock. Against the per-account limit (24 M as documented, 40 M per the U16 desk check, AUDIT.md §8) | OWNER-M7-8 (a separate fee payer for crank batches: keeper only) | open |
| KI-37 | OWNER-M7-4: an isolated missed covered second counts toward degraded mode only as part of 3 consecutive misses. `liq_cover_s` is 10 s | M4G-1 class: each miss adds one `L_ema` step | OWNER-M7-4 (PROPOSED until production telemetry) | open |
| KI-38 | OWNER-M7-9: the keeper makes its RPC calls one after another | One instance keeps ≈ 650 / 365 / 123 / 69 / 27 markets observed every second at 5 / 10 / 30 / 50 / 100 ms RTT (derived; UNVERIFIED on a live RPC) | OWNER-M7-9 | open (accept for the beta) |
| KI-39 | Creator-ground mints: pump derives mint-dependent PDAs on every v2 trade (M5-SEC-6 residual, ARCH §6.1) | ≈ 1,500 CU per extra bump. The worst measured curve open is 199,945 of the PROPOSED 250,000 CU | O17 sub-item | disclosed |
| KI-40 | OWNER-M7-10 (FUZZ-M7-1): a donation to a curve market's `trader_wsol` is not swept by curve legs | No funds at risk. The lamports go to the lenders through `sweep_trader` or the first PumpSwap leg | OWNER-M7-10 | disclosed (resolved as built; confirm) |
| KI-41 | Transaction size. The PumpSwap open with 4 extras and a separate fee payer goes over v0 through A28 (OWNER-M7-1). `initialize_config` as a legacy transaction with a separate fee payer also goes over, and cannot be sent through a Squads vault (OWNER-E2E-1) | Open: 1,238–1,239 B through A28; 1,207–1,208 B through the build-alt table. `initialize_config`: 1,294 B; 1,198 B self-paid | OWNER-M7-1, OWNER-E2E-1 | open (no program change) |

## 4. Trust assumptions and venue exposure

| ID | Statement | Bound | Decision | Status |
|---|---|---|---|---|
| KI-42 | The venue guard pauses opens only, and only once the latch is set. An open that lands between a venue upgrade and `flag_venue_upgraded` is accepted (ARCH §8.5; U26) | The keeper's reaction time, normally the next slot. LiteSVM does not model loader delay visibility | ADOPTED Q13 | by design |
| KI-43 | Healthy positions on a halted or route-broken venue can be neither closed nor settled until the route returns, an upgrade adapts the adapter (48 h), or their deadline passes (ARCH §8.5, O14) | At most the tier term (≤ 30 d). Liquidatable positions settle through S3 after `s3_wait_s` | O14 (standing: disclosed for the MVP; a borrower-initiated SOL close in v1.1) | disclosed |
| KI-44 | Upgradeable venues. pump, pAMM and pump-fees share upgrade authority `7gZuf…` and upgrade several times a month. Raydium AMM v4 and CPMM use a Squads v4 multisig (3 of 4, no timelock) and have no verified builds; their pool admin can halt pools and raise fees. Token-2022 is upgradeable by `AeLmXC…`. A malicious venue upgrade can harm exits (SPEC R5) | No design bound. The latch pauses opens; fee caps of 150 / 500 bps; the extras sets and S3 handle interface changes. CPMM was redeployed at slot 452,376,460 and Jupiter at 451,957,263 (M10 plan) | SPEC §12 R5; O6 | disclosed. CPMM re-validation TODO(WP-2) |
| KI-45 | short.win and `complete_curve` are upgradeable by the Governance multisig (48 h). The Squads v4 semantics to disclose: Emergency cannot veto upgrades; approved Squads transactions never expire; Governance cannot cancel reactively inside the program window; no spending limits | One program timelock of 48 h. Under Q4 option A, ≈ 96 h per change (48 h Squads + 48 h program) | Q4 (authority layout) | TODO(WP-7): the T-SQ-7 evidence and the layout the owner records |
| KI-46 | Admin powers move no funds. Emergency has no timelock: it can pause opens, disable a venue for opens, revalidate a venue, set the route-broken flag and bounded extras (O6), and clear (never set) a coverage pause (ARCH §3.1, I4) | The I4 invariant after every instruction. `sweep_protocol_fees` moves at most `protocol_fees`, and only to `fee_recipient` | SPEC §11; O6; M7-3 | by design |

## 5. Platform limits and external facts relied on (ARCH §12)

| ID | Statement | Bound | Decision | Status |
|---|---|---|---|---|
| KI-47 | Per-writable-account CU limit (U16, C0-17). Every exit and crank write-locks `Market`, so a censoring attacker must fill a market's per-account budget every slot. LiteSVM enforces no block limit | 24 M CU as documented. The M10 desk check (AUDIT.md §8) found that the Agave master and Firedancer sources at the pinned commits scale it to 40 M under `raise_block_limits_to_100m`, which has been active on mainnet since slot 435,888,000 | ARCH §6.6 | disclosed. Doc update TODO(WP-11) |
| KI-48 | `CoverageGap` and the exit-path `MintSafetyTripped` are `emit!` log events, which are lost if the logs truncate (A69, IX-3) | Flagged per transaction by the indexer | A69 | by design |
| KI-49 | `withdraw` to a memo-required Token-2022 account relies on the memo instruction right before `transfer_checked` (U17). Any CPI added between the two would break memo-required withdrawals | Pinned by `security::sec_7_*` | T-MEMO | TODO(WP-3): verify on the mainnet Token-2022 ELF |
| KI-50 | The pump adapter relies on pump's undocumented trailing cap byte on buys: no byte means a revert above `rT` (U29) | Pinned 0 / 1 / 2 / none on the dumped ELF (`pump_math_it`). Re-checked on every pump upgrade (the latch) | ARCH §12 U29 | disclosed |
| KI-51 | Keeper batches are v1 transactions, and v1 landing on mainnet is unverified (U25). Jito bundle semantics against a live block engine are unverified too (U22, U8) | v0 fallback: 3 / 4 / 4 / 5 cranks (PumpSwap / AMM v4 / CPMM / curve) per transaction | Q6 (a): probe spend, or M11 | open |
