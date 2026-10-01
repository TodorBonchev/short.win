# short.win — Product & Implementation Specification

*v0.3 • Solana • Permissionless P2P token lending for shorting memecoins*
*Supersedes "ShortMeme" v0.1 and short.win draft v0.2. Domain: **short.win** • Brand: **ShortWin***

---

## 0. What changed

### v0.2 → v0.3 (2026-09-27)
All draft decisions marked "⚠ Owner confirm" in v0.2 were confirmed by the owner:

- **Sale proceeds stay locked in the position vault** (§2.1).
- **Duration:** open-ended with a per-tier max duration (§4, Q15).
- **Mint safety filter:** automatic rejection of freeze authority, permanent delegate, transfer hooks and transfer fees (§3).

Decided by the owner after the venue research and design review (docs/research/, docs/design/):

- **Graduated-token execution (amends #16):** trade the token's canonical constant-product pool directly — PumpSwap for pump.fun graduates, Raydium AMM v4 / CPMM otherwise. Jupiter moves to v1.1 as an optional better-price path, bounded by the on-chain pool quote (§2.1, §3, §9).
- **Raydium in the MVP:** tokens whose main SOL pool is Raydium AMM v4 or CPMM (pre-PumpSwap pump.fun graduates, LaunchLab, most older memecoins) can be shorted at launch. Tokens that trade only on concentrated-liquidity venues (Orca Whirlpools, Meteora DLMM, Raydium CLMM) can be listed and lent, but not shorted, until a later version (§3).
- **SOL settlement fair-value cap (amends §5.3):** S1 is unchanged. In S2 and S3 the lenders' take is capped at fair value and any excess returns to the borrower (§5.3).
- **Margin rule (clarifies §4):** initial margin is whatever makes the opening CR in the table hold after venue fees, checked at both spot and EMA price (§4).

Decided by the owner after the third and fourth design reviews (docs/design/ARCHITECTURE.md v0.4.1 §10):

- **Lender SOL income (O16, clarifies §2.4):** rent is credited to lenders continuously; one-off lender income vests over 6 hours (§2.4).
- **Emergency venue powers (O6, amends §9/§11):** a "route broken" flag and a bounded set of extra venue accounts, neither of which can move funds (§11).
- **Expired positions in S2/S3 (O15, clarifies §5.3):** the fair-value cap charges the 2% expiry fee, not the liquidation penalty (§5.3).
- **Liquidator floor (N22, clarifies §6.3):** a liquidating keeper earns at least the SOL-settlement keeper reward, topped up from the borrower's remainder (§6.3).

### v0.1 → v0.2

| Area | v0.1 | v0.2 |
|---|---|---|
| Name | ShortMeme | **short.win / ShortWin** |
| Token coverage | Curated, bonding curve later | **All Solana SPL tokens, including Pump.fun tokens still on the bonding curve, from day one** |
| Listing | Open question | **Fully permissionless** — anyone can create a market for any mint |
| Bad debt | Open question | **Lender bears it.** No insurance fund, no treasury backstop |
| Liquidation settlement | Open question | **Lender receives SOL (collateral + fees) instead of tokens** when buyback isn't possible |
| Short execution | Open question | **short.win executes the sell** — Pump.fun bonding curve if not graduated, Jupiter if graduated |
| Collateral, fees, rates | Open | Proposed values in §4–§6 |

---

## 1. Purpose

short.win lets anyone short any Solana token — including brand-new Pump.fun launches still on the bonding curve — by borrowing it from lenders who earn rent.

- **Lender** deposits tokens into a per-token pool and earns rental fees. Accepts that if a short can't be bought back, they are repaid in SOL instead of tokens.
- **Borrower** posts SOL margin; short.win borrows the tokens and sells them in the same transaction. Borrower profits if the price falls.
- **Protocol** creates markets permissionlessly, prices risk automatically, executes trades, liquidates unsafe positions, and collects fees.

**Core question:** can idle memecoin inventory be turned into borrowable short liquidity, with rules clear enough that lenders knowingly accept the risk?

---

## 2. Core flows

### 2.1 Open a short (atomic, one transaction)
1. Borrower picks a token and quantity `Q`, sets max slippage.
2. Borrower deposits SOL margin `M`.
3. Program borrows `Q` from the pool and sells it:
   - **Bonding curve** (not graduated): directly into the Pump.fun curve.
   - **Graduated**: directly into the token's canonical constant-product pool — PumpSwap for pump.fun graduates, Raydium AMM v4 / CPMM otherwise. ✅ Owner (v0.3). Jupiter is added in v1.1 as an optional better-price path, bounded by the on-chain pool quote.
4. Sale proceeds `S` **stay locked in the position vault** alongside `M`. ✅ Owner confirmed (v0.3)
5. Position records: `Q` owed, `S`, `M`, entry price, open time, tier, rate.

Keeping `S` in escrow is the single most important safety choice in this doc: collateral starts at `M + S` instead of just `M`, so the lender is protected by the sale proceeds plus the margin. The borrower never holds the tokens or the proceeds, so there's nothing to "run with."

### 2.2 Close a short
1. Borrower requests close with max slippage.
2. Program uses vault SOL to buy back `Q` tokens (curve or Jupiter, whichever is live).
3. `Q` tokens return to the pool; rent and fees are paid.
4. Remaining SOL goes to the borrower. Profit if buyback cost < `S`.

### 2.3 Liquidation — see §5.

### 2.4 Lend
1. Lender deposits tokens into the market's pool and receives pool shares.
2. Rent accrues to the pool continuously.
3. Lender withdraws pro-rata from **unborrowed** inventory. If the pool has received SOL from settlements (§5.3), withdrawals are a pro-rata mix of tokens and SOL.
4. One-off lender income — the lenders' share of liquidation penalties, expiry fees, settlement proceeds above the market value of the debt, and swept balances — **vests to lenders linearly over 6 hours** (configurable), so a deposit made just before such a payment cannot capture it. Rent is not vested; it is credited continuously as it accrues. ✅ Owner (v0.3, O16)

---

## 3. Markets & permissionless listing

- **Anyone can create a market** for any SPL mint by paying a small creation fee (covers rent for accounts; suggest ~0.02 SOL).
- No admin approval. Risk tier is **assigned automatically from on-chain state** (§4) and re-evaluated on every interaction.
- **Mint safety filter (automatic, not curation).** ✅ Owner confirmed (v0.3). Reject mints that could make the lender vault unsafe regardless of price:
  - active **freeze authority** (issuer could freeze the vault),
  - Token-2022 **permanent delegate** (issuer could pull tokens out of the vault),
  - Token-2022 **transfer hooks** or **transfer fees** (break accounting/CPI).
  Standard Pump.fun tokens pass this filter. This keeps listing permissionless while blocking tokens that are mechanically broken for lending.
- Supported venues (MVP): **Pump.fun bonding curve**, **PumpSwap** (canonical pool of pump.fun graduates), and **Raydium AMM v4 / CPMM** SOL pools. ✅ Owner (v0.3). Jupiter follows in v1.1.
- Tokens whose SOL liquidity is only on concentrated-liquidity venues (Orca Whirlpools, Meteora DLMM, Raydium CLMM) can be listed and lent at launch, but opening shorts on them waits for a later version that can price those pools on-chain.

---

## 4. Risk tiers & collateral

Tiers are computed on-chain from venue state and liquidity. Collateral ratio **CR = (vault SOL) ÷ (SOL cost to buy back Q now)**, where buyback cost is the *executable* cost including price impact, not spot × Q.

| Tier | Criteria | Initial margin (of notional) | Opening CR | Liquidation CR | Price rise to liquidation | Liq. penalty | Max duration |
|---|---|---|---|---|---|---|---|
| **A** | Graduated, pool liquidity ≥ $500k, age ≥ 7 days | 100% | 200% | 125% | ~1.6× | 5% | 30 days |
| **B** | Graduated, liquidity $100k–500k | 150% | 250% | 135% | ~1.85× | 7% | 14 days |
| **C** | Graduated, liquidity < $100k or graduated < 24h ago | 200% | 300% | 150% | ~2.0× | 8% | 7 days |
| **D** | Still on Pump.fun bonding curve | 300% | 400% | 175% | ~2.3× | 10% | 72 hours |

*"Price rise to liquidation" ≈ Opening CR ÷ Liquidation CR, ignoring rent accrual.*

**Pool age ✅ Owner (v0.3, O13).** A graduated pool's age (the "graduated < 24h ago" and "age ≥ 7 days" criteria) is measured from the moment short.win first observes it: the real graduation time for markets listed before their token graduated, otherwise the market's listing time. Raydium AMM v4 and PumpSwap pools (and CPMM pools created before mid-2024) store no provable creation time, so an old pool listed today starts in Tier C for 24 h and is at most Tier B until day 7. This errs on the conservative side; the UI explains the tier. A Raydium CPMM pool created after mid-2024 does record a provable creation time (`open_time`), and its age is counted from that. ✅ Owner (2026-09-28): accepted knowing a pool can be created early and listed later to start older, because a pool's tier liquidity is bounded by its locked depth.

**Margin rule ✅ Owner (v0.3).** The program enforces the **Opening CR** column: margin `M` is whatever makes (M + S) ÷ executable buyback cost ≥ Opening CR, checked at both the spot and the EMA price. The "Initial margin" column is the approximate result before venue fees; borrowers post about 2% more than it once fees (~1.25% each way) are included.

### Size caps (all tiers)
- **Per position:** buyback of `Q` must move price ≤ 5% (Tier D: ≤ 3%) at open time.
- **Per market (total open shorts):** total `Q` outstanding ≤ amount whose buyback would move price ≤ 20% (Tier D: ≤ 10%).
- **Utilization cap:** ≤ 90% of pool inventory borrowed.
- **Per wallet:** ≤ 25% of a market's open interest.

Caps are derived from live curve/pool reserves, so they grow automatically as a token gains liquidity.

### Duration ✅ Owner confirmed (v0.3, Q15)
Positions are **open-ended up to the tier's max duration**. After max duration, the position becomes liquidatable at no penalty to the lender side (borrower pays a 2% expiry fee instead of the full liquidation penalty). Reason: lenders need a guaranteed horizon, and bonding-curve tokens can go dead within days.

---

## 5. Liquidation & SOL settlement

### 5.1 Trigger
A position is liquidatable when **any** of these holds:
1. **CR < tier's liquidation CR**, confirmed on both the current executable price **and** a short-window EMA price (prevents one-block manipulation from liquidating someone).
2. **Max duration exceeded.**

Anyone can call liquidate (permissionless keepers). short.win runs its own keeper as a backstop.

### 5.2 Normal liquidation (token buyback)
1. Keeper triggers; program buys back `Q` using vault SOL, bounded by max slippage.
2. `Q` tokens + accrued rent return to the pool.
3. Penalty is taken from the remaining SOL (split per §6.3).
4. Anything left returns to the borrower.

### 5.3 SOL settlement (lender takes collateral instead of tokens)
When a normal buyback isn't possible, the **position settles in SOL** and the token debt is extinguished:

- **S1** (a real gap): the lender pool receives **all vault SOL minus the keeper reward**. The borrower receives nothing.
- **S2 / S3** (thin liquidity or no route): the lender pool receives **all vault SOL minus the keeper reward, capped at fair value** = executable cost to buy back `Q` at the EMA price + liquidation penalty + accrued rent. Any excess goes back to the borrower. ✅ Owner (v0.3). Reason: without the cap, lenders profit from settling healthy positions, which rewards them for draining liquidity or halting routes.
  - For a position settled in S2 or S3 only because it **expired** (it is not below its liquidation CR), fair value uses the **2% expiry fee** instead of the liquidation penalty, matching §4. ✅ Owner (v0.3, O15; the same reasoning applies to S2 as to S3)

SOL settlement applies **only** under these exact conditions:

| # | Condition | Typical cause |
|---|---|---|
| S1 | Executable cost to buy back `Q` **exceeds vault SOL** minus keeper reward | Price spiked faster than liquidation could run (gap) |
| S2 | Buying back `Q` would exceed **max liquidation slippage** (20%; Tier D 30%) | Liquidity too thin at that moment |
| S3 | **No executable route** for ≥ 10 minutes while the position is liquidatable | Venue halted, pool drained, migration stuck |

Rules:
- The program must **attempt the normal buyback first** (or prove via on-chain quote that it would fail) before S1/S2 settlement is allowed.
- **Graduation in progress:** while a Pump.fun token is migrating from curve to AMM, liquidations pause and the S3 timer doesn't start until 10 minutes after migration completes.
- SOL received by a pool is distributed **pro-rata to all lenders in that pool** (pools are shared, so losses are shared).
- UI must disclose on the deposit screen, before signing: *"If a borrower can't be bought back, you will be repaid in SOL, which may be worth less than the tokens you lent."*

### 5.4 Bad debt
**Lenders absorb it**, pro-rata within the affected market. No insurance fund, no treasury backstop. Markets are fully isolated — a loss in one token's pool cannot touch any other pool.

---

## 6. Fees & rates

### 6.1 Rental rate — dynamic, utilization-based (kinked)
```
if U ≤ 80%: rate = base + slope1 × U
if U > 80%: rate = base + slope1 × 80% + slope2 × (U − 80%)
```

| Tier | Base (APR) | Rate at 80% util | Rate at 100% util |
|---|---|---|---|
| A | 10% | 50% | 250% |
| B | 25% | 100% | 400% |
| C | 50% | 200% | 800% |
| D | 100% | 400% | 1,500% |

- **Minimum rent:** 1 hour of rent (Tier D: 4 hours), so flash open/close can't borrow for free.
- Rent is denominated in SOL, accrued per second on current notional, paid from the vault on close/liquidation.

### 6.2 Protocol fees
- **Open fee:** 0.25% of notional (protocol).
- **Close fee:** none.
- **Market creation:** ~0.02 SOL.

### 6.3 Splits

| Fee | Lenders | Protocol | Keeper/liquidator |
|---|---|---|---|
| Rent | 85% | 15% | — |
| Liquidation penalty | 50% | 20% | 30% |
| SOL settlement | all remaining SOL | — | fixed reward (0.01 SOL + 1% of vault, capped) |
| Expiry fee (2%) | 100% | — | — |

No reserve/insurance allocation, consistent with lenders bearing bad debt.

**Liquidator floor ✅ Owner (v0.3, N22).** A keeper that liquidates a position normally (§5.2) is paid at least what it would have earned from a SOL settlement of the same position (0.01 SOL + 1% of vault, capped). Any top-up above its 30% penalty share comes from the borrower's remainder. Without it, on small positions a keeper earns more by letting the position gap into a lender loss than by liquidating it.

---

## 7. Lender proposition

- **Earn rent** on tokens that would otherwise sit idle.
- **Isolated risk:** only exposed to the one token they lend.
- **Explicit, signed-off downside:** if the token pumps faster than liquidation can buy back, they receive SOL (collateral + penalty + rent) rather than tokens.
- **Creators:** identical treatment to any lender. No badges or special status at launch. (The UI may still show the fact that a deposit came from the mint's creator wallet, since it's public on-chain data — see risk R1.)

Lending is not staking and not risk-free.

---

## 8. MVP scope

**In:**
- Wallet connect, token search by mint/ticker, permissionless market creation
- Lender deposit / withdraw (token + SOL mix)
- Atomic open-short via the Pump.fun curve, PumpSwap, or Raydium AMM v4 / CPMM
- Close, liquidation, SOL settlement, expiry
- Position dashboard: `Q` owed, vault SOL, CR, liquidation price, P&L, rent accrued, time left
- Market page: tier, inventory, utilization, rate, open interest, caps, recent liquidations
- Keeper bot (in-house) + open liquidation instruction
- Indexer/API: markets, deposits, positions, closes, liquidations, settlements, fees
- Analytics: TVL, utilization, OI, fees, liquidations, SOL-settlement count and lender loss

**Deferred:** cross-token collateral, leverage beyond margin, creator status, governance, social/copy trading, limit orders, partial closes (consider for v1.1).

---

## 9. Technical architecture

- **On-chain program (Anchor):** markets, pools, positions, vaults, open/close/liquidate/settle, fee accounting, tier computation.
- **Venue adapters (CPI):** Pump.fun bonding curve (buy/sell, curve math on-chain); PumpSwap and Raydium AMM v4 / CPMM for graduated tokens, with reserves read from the same pool for on-chain pricing; Jupiter in v1.1.
- **Pricing:** on-chain executable-cost computation from curve/pool reserves + an EMA price updated on every interaction and by keeper crank.
- **Indexer/backend:** event indexing, REST/WebSocket API, market analytics.
- **Keeper:** monitors all positions, liquidates, handles expiry and SOL settlement.
- **Frontend (short.win):** fast trade UI, transaction simulation before signing, clear risk disclosures.
- **MEV:** submit open/close/liquidate with tight slippage and via bundles where available to limit sandwiching.
- **Security:** isolated pools, strict account constraints, checked math, admin keys limited to emergency pause and fee parameters (no ability to move user funds), fuzzing and invariant tests, external audit before mainnet.

---

## 10. On-chain state

- **GlobalConfig:** fee recipient, pause authority, fee parameters, tier parameter table, venue program IDs.
- **Market:** mint, venue state (curve / graduated + pool), tier (cached), pool vault (tokens), pool SOL vault, total shares, total borrowed `Q`, open-interest cap, EMA price, rate index, paused flag.
- **LenderPosition:** owner, market, shares.
- **ShortPosition:** borrower, market, `Q` owed, vault SOL (margin + proceeds), entry price, open timestamp, max-duration deadline, rate index at open, status.
- **Events:** MarketCreated, Deposit, Withdraw, Open, Close, Liquidate, SolSettle, Expire, FeeCollected.

### Key invariants
- Tokens in pool vault + `Q` outstanding across positions = total lender token claims (until SOL settlement converts some claims to SOL).
- A position's vault SOL can only leave via close, liquidation, or SOL settlement.
- No instruction lets an admin withdraw pool or position funds.
- Utilization never exceeds cap after an open.

---

## 11. Emergency controls

- **Pause new opens** globally or per market (keeps closes and liquidations running).
- **Keeper coverage pause ✅ Owner (v0.3, O17).** A separate keeper key, the `coverage_authority`, may pause and resume new opens on one market when the in-house keeper cannot keep that market covered (degraded mode: an outage, an exhausted per-market budget), with an alert. It moves no funds and never pauses closes, liquidations, settlements, withdrawals or claims; it is inert until governance installs the key with a timelocked authorities change, and only that key can set or clear its flag. A clear lets opens through only a few seconds later (5 s), so the key cannot slip an open past the pause within one transaction (M7-SEC-3).
- **Never pause closes or liquidations** except during a detected venue migration (§5.3).
- Pause authority is a multisig; fee/tier parameter changes are timelocked (suggest 48h).
- **Venue-breakage powers ✅ Owner (v0.3, O6; amends §9's "emergency pause and fee parameters" list).** The emergency multisig may also, without timelock:
  - set a **"route broken"** flag on a venue or market, which refuses new opens and starts the §5.3 S3 timer — it never pauses closes or liquidations and moves no funds;
  - set a **bounded list of extra accounts** passed to a venue's instructions after that venue changes its interface, so exits keep working without a program upgrade. Callers can always choose the previous or empty set, so a bad setting cannot block exits, and no setting can move funds.
- **Emergency cancel of governance changes ✅ Owner (v0.3, M1-SEC-1).** The emergency multisig may cancel a pending *parameter* change during its timelock (a fast check on a compromised governance multisig), but not a change of the authorities or of the timelock itself, so governance can always rotate a compromised emergency key once the timelock has run.

---

## 12. Key risks to design against

| # | Risk | Mitigation |
|---|---|---|
| R1 | **Creator self-dealing:** creator lends supply, shorts it from another wallet, sells into the curve — a disguised dump. If the token later pumps, they may get SOL settlement on their own lending. | Show creator-wallet deposits on the market page; per-market OI and notional caps tied to liquidity (the bound the risk analysis relies on). Per-wallet caps are a UX limit only: they are per key, so one actor with four wallets fills a market's OI cap (M7-SEC-9). Can't be fully prevented in a permissionless design — disclose it. |
| R2 | **Liquidation manipulation:** attacker pumps a thin token for one block to force liquidations. | EMA + executable-price dual check; liquidation slippage bounds. |
| R3 | **Gap risk:** 5–10× moves in minutes on bonding-curve tokens. | Tier D margins, tiny caps, SOL settlement; lenders pre-accept. |
| R4 | **Sandwiching opens/closes.** | Tight slippage, bundles. |
| R5 | **Venue changes** (Pump.fun curve/migration program upgrades). | Adapters versioned; pause opens for a market on unknown program state. |
| R6 | **Regulatory.** A public shorting/lending product may be treated as a regulated financial product in some jurisdictions. | Get legal advice before launch; consider geo-restrictions on the frontend. |

---

## 13. Success metrics

Lendable inventory; utilization; unique lenders/borrowers; OI; rental fees; protocol revenue; avg. duration; close vs. liquidation vs. SOL-settlement rate; **realized lender loss** (SOL settlement value vs. token value at settlement); avg. execution slippage; repeat lenders/borrowers.

---

## 14. Decision log

| # | Question | Decision | Status |
|---|---|---|---|
| 9 | Bonding-curve tokens at launch? | Yes — all tokens, including un-graduated Pump.fun | ✅ Owner |
| 10 | Collateral & liquidation by tier | §4 table | ✅ Delegated, proposed |
| 11 | SOL settlement for lenders? | Yes, under conditions S1–S3 (§5.3) | ✅ Owner; conditions proposed |
| 12 | Who absorbs bad debt? | Lenders, pro-rata per market | ✅ Owner |
| 13 | Fixed or dynamic rent? | Dynamic, kinked utilization curve (§6.1) | ✅ Delegated, proposed |
| 14 | Creator special status? | No, identical treatment | ✅ Owner |
| 15 | Fixed vs open-ended duration? | Open-ended with per-tier max (72h–30d) | ✅ Owner (v0.3, 2026-09-27) |
| 16 | Venues | Pump.fun curve + PumpSwap + Raydium AMM v4 / CPMM, traded directly; Jupiter in v1.1 (was: curve + Jupiter) | ✅ Owner (amended v0.3, 2026-09-27) |
| — | Tokens only on concentrated-liquidity venues (Orca, Meteora DLMM, Raydium CLMM) | Listable and lendable at launch; shorting later | ✅ Owner (v0.3, 2026-09-27) |
| — | SOL settlement fair-value cap | S1 unchanged; S2/S3 lenders' take capped at fair value, excess to borrower (§5.3) | ✅ Owner (v0.3, 2026-09-27) |
| — | Margin rule | Enforce the table's Opening CR after fees, at spot and EMA (§4) | ✅ Owner (v0.3, 2026-09-27) |
| O16 | Lender SOL income | Rent credited continuously; one-off lender income vests over 6 h (§2.4) | ✅ Owner (v0.3, 2026-09-27) |
| O6 | Emergency venue-breakage powers | Route-broken flag + bounded venue extra accounts; no fund movement (§11) | ✅ Owner (v0.3, 2026-09-27) |
| O15 | Fair value for expired positions in S2/S3 | 2% expiry fee, not the liquidation penalty (§5.3) | ✅ Owner (v0.3, 2026-09-27) |
| N22 | Liquidation keeper pay on small positions | Floor at the SOL-settlement keeper reward, topped up from the borrower remainder (§6.3) | ✅ Owner (v0.3, 2026-09-27) |
| M1-SEC-1 | Emergency veto over pending governance changes | Parameter changes only; authorities and timelock changes can be cancelled only by governance (§11) | ✅ Owner (v0.3, 2026-09-27) |
| O13 | Pool age for tier criteria when the pool's creation time is not provable | Counted from short.win's first observation (graduation or listing) (§4) | ✅ Owner (v0.3, 2026-09-28) |
| O13b | Age of CPMM pools with a provable `open_time` (after mid-2024) | Counted from `open_time`, not first observation (§4; M3-SEC-4) | ✅ Owner (v0.3, 2026-09-28) |
| M4R-1 | Exits right after a Raydium pool switch | Redesigned before M5: the switch reads no single-transaction sample; exits wait for 8 s of the new pool's own observations ("post-change confirmation") | ✅ Owner (v0.3, 2026-09-28) |
| #99 | Pool switches that carry open positions | Only onto a pool whose normal opening caps every open position fits; otherwise positions exit through S3 at fair value | ✅ Owner (v0.3, 2026-09-29) |
| FITS-2 | Liquidity pushes by unlocked-LP holders on Raydium | Liquidation's spot check reads max(live, smoothed) liquidity; keeper observes every second while liquidity anomalies recur on markets with positions | ✅ Owner (v0.3, 2026-09-29) |
| FITX-1 | A genuine short withdrawal whose low reading is then frozen | Accepted as a bounded residual: it forces only the liquidation the same LP gets by withdrawing for good (same second, same price, borrower no worse off) | ✅ Owner (v0.3, 2026-09-29) |
| M4F-1 | Pool switch that would carry open positions (Raydium fast path; a list-only market with positions) | Switch only if every open position fits the new pool's normal opening size caps (§4); if no pool fits, positions stay on the failed pool and exit through S3 at fair value (§5.3) | ✅ Owner (v0.3, 2026-09-29) |
| M6R-1 | Third-party liquidity on canonical PumpSwap pools (anyone can deposit and withdraw) | The Raydium liquidity-push protections apply to PumpSwap: liquidation's spot check at max(live, smoothed) liquidity; the keeper's per-second coverage of liquidity anomalies on markets with positions; the pool's locked depth taken from its burned LP bounds sizing, tier and the exit reference, as on Raydium | ✅ Owner (v0.3, 2026-09-30) |
| M6-SEC-1 | The price seed after a venue change or graduation reads the lowest recorded price | The seed may fall by at most one normal smoothing step per second, as it may rise; no exit is forced by a seed that lags a fall | ✅ Owner (v0.3, 2026-09-30) |
| M6-SEC-3 | New curve positions while PumpSwap has disabled pool creation (graduation would stall) | Refuse new opens on the curve while that switch is set; existing positions unaffected | ✅ Owner (v0.3, 2026-09-30) |
| M6R-4 | Exit delay right after graduation | Accepted: keeper exits on a graduated position come 8–12 s after the migration (the new pool's prices are confirmed first) | ✅ Owner (v0.3, 2026-09-30) |
| M6D-1 | Sizing on PumpSwap while third-party liquidity sits in the pool (a position sized on a deposit that is withdrawn as a pump starts can end in lender loss) | PumpSwap is sized on its burned (permanently locked) liquidity only: position size, market caps and the tier count none of a third party's deposit; Raydium keeps sizing on its locked liquidity over the 50 % minimum. Every other place that credits the locked liquidity over the minimum takes 100 % on PumpSwap as well: the exit reference's depth limit is the burned liquidity itself, so a deposit withdrawn at a position's deadline cannot hold its expiry up (the #106 verification's M6E-1; ECON #106, §12.12 (12) (f)) | ✅ Owner (v0.3, 2026-09-30) |
| O17 | In-house keeper operating budget | Targeted coverage of every market with open positions (per-second only when needed: near thresholds, price/liquidity anomalies, warm-ups, guards; otherwise ~30 s); per-market budget whose exhaustion puts that market in degraded mode (opens paused automatically via the pause-opens-only coverage_authority, alert; exits continue); priority-fee ceiling 100,000 µlamports/CU | ✅ Owner (v0.3, 2026-09-30) |
| 17 | Who executes the short? | short.win, atomically at open | ✅ Owner |
| — | Sale proceeds kept in position vault | Yes | ✅ Owner (v0.3, 2026-09-27) |
| 18 | Fee split | §6.3 | ✅ Delegated, proposed |
| 19 | Listing | Permissionless | ✅ Owner |
| — | Mint safety filter (freeze/permanent delegate/hooks/fees) | Reject automatically | ✅ Owner (v0.3, 2026-09-27) |
| 20 | Launch | Fully permissionless | ✅ Owner |

---

## 15. Agent implementation brief

Treat this document as the baseline. Don't invent economics; any new parameter must be configurable and listed in the decision log.

- **Protocol agent:** accounts, instructions (create_market, deposit, withdraw, open_short, close_short, liquidate, sol_settle, expire, crank_price), state transitions, invariants in §10.
- **Economic/risk agent:** formalize CR, tier assignment, caps, rate curve, S1–S3; simulate 2×/5×/10× gaps, liquidity drains, and graduation mid-position; report expected lender loss per tier.
- **Application agent:** lender, borrower, market, and position UX; risk disclosures; indexer API.
- **Security agent:** threat model (§12), invariant/property tests, fuzzing of curve math and liquidation paths, adversarial scenarios (manipulation, self-dealing, MEV).
