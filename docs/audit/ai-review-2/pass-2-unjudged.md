# Pass 2 of the scan: raw agent reports, not judged

Pass 2 stopped after 34 of its 84 agents had returned, and its judge step never ran. This file lists what those 34 agents reported, as they wrote it. Nothing here was deduplicated, checked against the four gates or given a confidence. Read every block as a lead. The agents had pass 1's results as known ground ([`report.md`](report.md)) and reused its labels, so most blocks restate a pass-1 item.

Local paths in the agents' text are replaced with `<scratch>`, `<checkout>`, `<audited tree>` or `<local path>`.

## Coverage

| Slice | Agents returned | Not covered in pass 2 |
|---|---|---|
| lending | 12 of 12 | — |
| positions | 12 of 12 | — |
| settlement | 9 of 12 | 2 access-control (a safety classifier stopped it; it returned no blocks), 11 trust-gap, 12 flow-gap |
| venues-pump | 0 of 12 | 1 math-precision, 2 access-control, 3 economic-security, 4 execution-trace, 5 invariant, 6 periphery, 7 first-principles, 8 asymmetry, 9 account-validation, 10 numerical-gap, 11 trust-gap, 12 flow-gap |
| venues-raydium | 0 of 12 | 1 math-precision, 2 access-control, 3 economic-security, 4 execution-trace, 5 invariant, 6 periphery, 7 first-principles, 8 asymmetry, 9 account-validation, 10 numerical-gap, 11 trust-gap, 12 flow-gap |
| governance | 0 of 12 | 1 math-precision, 2 access-control, 3 economic-security, 4 execution-trace, 5 invariant, 6 periphery, 7 first-principles, 8 asymmetry, 9 account-validation, 10 numerical-gap, 11 trust-gap, 12 flow-gap |
| markets | 0 of 12 | 1 math-precision, 2 access-control, 3 economic-security, 4 execution-trace, 5 invariant, 6 periphery, 7 first-principles, 8 asymmetry, 9 account-validation, 10 numerical-gap, 11 trust-gap, 12 flow-gap |

330 blocks: 62 FINDING, 268 LEAD.
- 50 blocks have a key that pass 1 did not record (section 1, in full).
- 3 blocks are FINDINGs on keys that pass 1 judged as leads (section 2, in full).
- 277 blocks restate a key of pass 1 (section 3, one line each).

A key is `program|function|bug-class`, normalised as the skill does it.

## 1. Keys that pass 1 did not record

#### `shortwin-math|claim-sol|hardening-inconsistent-time-base` — LEAD — `lending:12-flow-gap`

- Location: `crates/shortwin-math/src/pool.rs:1849`
- Description: claim_sol moves the lock calendar at the raw clock and leaves last_accrual_ts, so after a clock step back a lock holder's withdraw fails. Agave's update_clock prevents the step back today.
- Seam: execution×periphery
- Fix: Make lock_of treat a lock whose calendar day is below cal_day as expired.
- Code smells: claim_sol's touch runs cal_advance (pool.rs:761) at the raw clock and moves MarketVault.cal_day to day(now) + 1. claim_sol writes no Market, so last_accrual_ts stays behind. lock_of (pool.rs:1715) checks only now < lock_until and never compares the lock's day with cal_day. A later withdraw at market_now with add == 0 and a live lock calls cal_add(v, l.lock_until, term) (pool.rs:1849). live_slot returns None for a day below cal_day, so cal_add returns InvalidInput (pool.rs:833), which maps to InvalidParams.

```diff
--- a/crates/shortwin-math/src/pool.rs
+++ b/crates/shortwin-math/src/pool.rs
@@ -1715,5 +1715,10 @@
 pub fn lock_of(vault: &Vault, lender: &Lender, now: i64) -> Result<u128, MathError> {
-    if lender.lock_epoch != vault.pi_epoch || lender.lock == 0 || now >= lender.lock_until {
+    // The calendar already dropped a lock whose day ended (cal_day moved past it).
+    if lender.lock_epoch != vault.pi_epoch
+        || lender.lock == 0
+        || now >= lender.lock_until
+        || day_of(lender.lock_until)? < vault.cal_day
+    {
         return Ok(0);
     }
```

#### `shortwin-math|claim|pot-draw-against-receivable` — LEAD — `lending:12-flow-gap`

- Location: `crates/shortwin-math/src/pool.rs:2074`
- Description: claim_sol pays pot SOL against rent that is still a receivable, so the first claimant takes SOL that another lender needs for its realised deficit. ECON §9.4 states the draw formula, not this case.
- Seam: execution×first-principles
- Proof: Scratch harness on a copy of shortwin-math: A and B deposit 1e9 each, and two positions borrow 5e8 each. 10 d of 20 SOL/d rent are credited (accrue). J deposits 2e9, and then P1 repays 100 SOL (credited 100). J's touch moves its 50 SOL surplus to the pot. A and B each have pending 100 SOL (50 from P1 + 50 from P2, still receivable) and real 25 SOL. A claims first: draw = min(50, 75) = 50, so A is paid 75 SOL. B's claim then draws 0 and is paid 25 SOL, so B's 25 SOL deficit of realised P1 rent went to A against P2's unpaid rent. P2 then exits with no rent (credited 200, paid 0). With A out of its free tokens, the final state is: A claimed 75 and holds pending +21, B claimed 25 and holds pending +43, J holds pending −64, which only J's cover pays.
- Fix: Exclude the lender's share of the open receivable from the pot draw, so the pot pays realised deficits first.
- Code smells: pool::claim sets need = pending − (real − claimed) (pool.rs:2073-2074). pending includes accrued rent that is still in Pool.receivable, so a receivable-only deficit draws from the pot. The pot holds realised surplus (a late lender's realised rent, covers) that lenders with realised deficits are owed. The draw order is first come. ECON §9.4 'Why a realised allocation' says a claim must not pay rent that is still a receivable, because the lender then escapes a later shortfall. The shortfall debit lands on the shares that hold at the exit (N37), so a claimant that also withdraws its free tokens keeps rent that defaults. cover_deficit lamports reach the same pot.

```diff
--- a/crates/shortwin-math/src/pool.rs
+++ b/crates/shortwin-math/src/pool.rs
@@ -2073,7 +2073,14 @@ pub fn claim(
     let own = sub_i128(i128::from(l.real), i128::from(l.claimed))?;
-    let need = sub_i128(i128::from(l.pending), own)?.max(0);
+    // C0-13: the pot pays realised deficits, not this lender's part of the unpaid receivable.
+    let recv = if l.epoch == pool.share_epoch && pool.total_shares != 0 {
+        to_i128(mul_div(u128::from(pool.receivable), l.shares, pool.total_shares, Rounding::Ceil)?)?
+    } else {
+        0
+    };
+    let need = sub_i128(sub_i128(i128::from(l.pending), own)?, recv)?.max(0);
```

#### `shortwin-math|deposit|vested-principal-diluted-by-late-deposit` — LEAD — `lending:4-execution-trace`

- Location: `crates/shortwin-math/src/pool.rs:1966`
- Description: deposit prices shares on token claims alone, so after a settlement cuts the claims a depositor buys most of the unreleased vest for few tokens. KI-24 and ECON §9.4 accept a holder through lump_vest_s, but state no bound for a share price that the settlement cut.
- Path: crates/shortwin-math/src/pool.rs
- Code smells: shares_for_tokens (pool.rs:1966) mints floor(amount*(S + VS)/(T + ΣB_Q + VA)) and ignores vest_bal. PoolDrained exempts ΣB_Q = 0, and a settlement flushes only at <= 1/1,000 of its claims. Trace (e035f2e): at the default 90 % cap, one 900,000 settlement with a 50 SOL lump leaves 100,000 of 1,000,000 claims, with no flush and no gate. J deposits 100,000 raw, about 5.6 SOL at P_ema. After 6 h J is paid 24,999,887,499 lamports and withdraws 99,999 tokens. At 1,001 claims left of 901,001 (no flush), a 1,001-raw deposit takes 24,987,532,589; at 900 left the vest flushes and J gets 0.

#### `shortwin-math|expire|hardening-rate-unchecked` — LEAD — `lending:8-asymmetry`

- Location: `crates/shortwin-math/src/waterfall.rs:301`
- Description: expire accepts any expiry_fee_bps, although its documentation says a rate above 10,000 bps returns InvalidInput.
- Path: crates/shortwin-math/src/waterfall.rs
- Proof: expire() calls check_bps(x.rent_protocol_bps) only. Then `let expiry_fee = bps_of(x.base(), expiry_fee_bps, Rounding::Ceil)?.min(w);`. The cap at w keeps the legs summing to V, and params bound ExpiryFeeBps to [0, 500]. Hardening only.
- Fix: Call check_bps(expiry_fee_bps) at the top of expire, as the documentation says.
- Code smells: Same documentation and check mismatch as liquidate.

```diff
--- a/crates/shortwin-math/src/waterfall.rs
+++ b/crates/shortwin-math/src/waterfall.rs
     check_bps(x.rent_protocol_bps)?;
+    check_bps(expiry_fee_bps)?;
```

#### `shortwin-math|liquidate|hardening-rate-unchecked` — LEAD — `lending:8-asymmetry`

- Location: `crates/shortwin-math/src/waterfall.rs:245`
- Description: liquidate accepts any penalty_bps, although its documentation says a rate above 10,000 bps returns InvalidInput.
- Path: crates/shortwin-math/src/waterfall.rs
- Proof: liquidate() calls split.validate() and check_bps(x.rent_protocol_bps) only. Then `let penalty = bps_of(x.base(), penalty_bps, Rounding::Ceil)?.min(w);`. The cap at w keeps the legs summing to V, and params bound TierPenaltyBps to [0, 2,000], so nothing breaks today. A future bound change would fail silently rather than with the documented InvalidInput.
- Fix: Call check_bps(penalty_bps) at the top of liquidate, as the documentation says.
- Code smells: The documented error contract differs from the checks the code runs.

```diff
--- a/crates/shortwin-math/src/waterfall.rs
+++ b/crates/shortwin-math/src/waterfall.rs
     split.validate()?;
     check_bps(x.rent_protocol_bps)?;
+    check_bps(penalty_bps)?;
```

#### `shortwin-math|liquidate|hardening-unchecked-rate` — LEAD — `settlement:8-asymmetry`

- Location: `crates/shortwin-math/src/waterfall.rs:241`
- Description: waterfall::liquidate and waterfall::expire do not check penalty_bps and expiry_fee_bps against 10^4, which their docs promise. Only the compiled parameter bounds hold them today.
- Code smells: The docs of waterfall::liquidate (waterfall.rs:235) and waterfall::expire (waterfall.rs:293) promise InvalidInput for a rate above 10^4 bps. Only rent_protocol_bps (and the split) is checked. penalty_bps and expiry_fee_bps are not. The compiled bounds keep them at or below 1,000 and 500 bps today.

```diff
--- a/crates/shortwin-math/src/waterfall.rs
+++ b/crates/shortwin-math/src/waterfall.rs
@@ -241,2 +241,3 @@ pub fn liquidate(
     split.validate()?;
     check_bps(x.rent_protocol_bps)?;
+    check_bps(penalty_bps)?;
@@ -298,1 +299,2 @@ pub fn expire(
     check_bps(x.rent_protocol_bps)?;
+    check_bps(expiry_fee_bps)?;
```

#### `shortwin-math|min-notional-buyback-room|buyback-room-assumes-zero-overshoot` — LEAD — `positions:10-numerical-gap`

- Location: `crates/shortwin-math/src/params.rs:559 (room term); floor at :543; consumer programs/shortwin/src/instructions/liquidate.rs:440`
- Description: At the min_notional floor a Tier A PumpSwap position has 0.165 % buyback room, so liquidate fails with UseSolSettle after any small price overshoot. We did not measure the real overshoot at the first liquidatable second in LiteSVM.
- Seam: three-way (boundary x precision x invariant): the compiled floor (boundary) is derived exactly at zero overshoot (precision of the derivation), so the invariant 'no in-bounds value may lose liquidations' fails for any real overshoot
- Path: crates/shortwin-math/src/params.rs
- Proof: Config, all in bounds and accepted by check_econ: Tier A at its SPEC minima (cr_open 20,000, cr_liq 12,500); settle_keeper_fixed 10,000,000 and settle_keeper_bps 100 (both the defaults and the compiled maxima); exec_rent_tol 10,000,000 (the maximum); min_notional = MIN_NOTIONAL_FLOOR = 53,000,000. On PumpSwap, open_short locks ert = exec_rent_tol (effective_ert). An open at N_open = 53,000,000 gives risk::margin_d4, V = 106,000,000 and risk::k_s = 11,060,000. liquidate::plan requires buyback_allowed, i.e. B_live + 10,000,000 <= V - K_s = 94,940,000, so B_live <= 84,940,000. The spot leg first holds at B_rs = floor(V*1e4/12,500) + 1 = 84,800,001, and B_rs <= B_live because the robust spot is min(P_live, lb_lo) and the spot liquidity is max(L_live, L_ema'). The window is therefore 139,999 lamports, 0.165 % of B. Measured with the math crate (scratch harness): Budget::exceeded_by allows the boundary B but refuses boundary + 0.5 %, while cr_below is true at the boundary. The EMA leg needs the newest liq_persist_n (2-3) ring entries, and EMA states lag spot by about (EMA_TAU_S + 1)*g (bounds.rs kappa_eff doc). In any pump faster than about 2.75 bps/s, B_live is already above 84,940,000 when the EMA leg turns true, so plan returns UseSolSettle (liquidate.rs:440). The position then waits for S1, which pays the keeper K_s (10.4 % of V) and the lenders V - K_s, and leaves the borrower 0 instead of about 6.5M lamports. In a fast pump the lenders also lose B - (V - K_s). R_due > 0 widens the window only by 0.8*R. At the same config Tiers B-D keep 13-57 %, and Tier A keeps 4.3 % at ert 6.5M. With delta_liq_bps at its maximum (500) as headroom, the same formula gives a floor of 66,666,670.
- Fix: Derive the floor with headroom for the liquidation's overshoot, for example the compiled maximum of delta_liq_bps, which raises the floor to 67,000,000 lamports.
- Code smells: The derivation in the MIN_NOTIONAL_FLOOR doc uses 'the cheapest CR-liquidatable buyback floor(V*1e4/cr_liq) + 1' as the execution cost. Liquidation also needs the lagging EMA leg, and it buys at the live quote, which is at or above the robust-spot quote. The AIR-06 regression test checks room only at the exact boundary.

```diff
--- a/crates/shortwin-math/src/params.rs
+++ b/crates/shortwin-math/src/params.rs
@@
-pub const MIN_NOTIONAL_FLOOR: u64 = 53_000_000;
+pub const MIN_NOTIONAL_FLOOR: u64 = 67_000_000;
@@ pub const MIN_NOTIONAL_BUYBACK_ROOM: u128 = {
     let fixed = Rule::SettleKeeperFixed.bounds().1 as u128;
     let bps = Rule::SettleKeeperBps.bounds().1 as u128;
     let ert = Rule::ExecRentTol.bounds().1 as u128;
+    // Headroom for the overshoot past the boundary: delta_liq_bps at its compiled maximum.
+    let delta = Rule::DeltaLiqBps.bounds().1 as u128;
     let mut floor = 0u128;
@@
-        let room = 10_000 * liq - 100_000_000 - bps * liq;
+        let room = 10_000 * liq - 10_000 * (10_000 + delta) - bps * liq;
```

#### `shortwin-math|ratio|hardening-zero-tag-full-wipe` — LEAD — `lending:10-numerical-gap`

- Location: `crates/shortwin-math/src/pool.rs:503`
- Description: ShareEpoch::ratio reads the all-zero tag as a full wipe, so a zeroed record sets a lender's shares to 0. Only load_records' discriminator and PDA checks stop such a record today.
- Seam: boundary x invariant: a type tag whose valid value equals zeroed memory
- Path: a ShareEpoch whose bytes are all zero -> ShareEpoch::ratio maps (num 0, den_log1000 0) to the full-wipe ratio (0, 1) -> convert_steps sets shares, lock and lock_term to 0 -> top_up would divide by s_old = 0
- Proof: pool.rs:501-506: `(0, 0) => Ok((0, 1))`, so the valid full-wipe tag equals zeroed memory. A full wipe in convert_steps scales shares, lock and lock_term to 0 (scale returns 0 when num == 0). s_old > 0 is only documented, and no reader checks it; top_up's divmod_floor(.., rec.s_old) returns DivByZero on a zeroed record. maybe_roll_in writes a record only while S > 0, so every real record has s_old > 0. Today load_records rejects a zeroed account (owner, Anchor discriminator, market and PDA checks), and sol_settle serializes each record in the instruction that allocates it, so no zeroed record reaches ratio.
- Fix: Refuse a record with s_old == 0 in ratio, because every written record has s_old > 0.
- Code smells: The full-wipe tag (num 0, den_log1000 0) equals zeroed memory, and the documented s_old > 0 is never checked on read.

```diff
--- a/crates/shortwin-math/src/pool.rs
+++ b/crates/shortwin-math/src/pool.rs
@@ -501,2 +501,6 @@ impl ShareEpoch {
     fn ratio(&self) -> Result<(u128, u128), MathError> {
+        // Every written record has s_old > 0: a zeroed record is never a full wipe.
+        if self.s_old == 0 {
+            return Err(MathError::InvalidInput);
+        }
         match (self.num, self.den_log1000) {
```

#### `shortwin-math|settle|vest-flush-per-settlement-ratio` — FINDING — `lending:4-execution-trace`

- Location: `crates/shortwin-math/src/pool.rs:2356`
- Description: settle tests each settlement alone for the 1/1,000 flush, so after two settlements reach dust a later depositor takes the old lenders' vest.
- Path: crates/shortwin-math/src/pool.rs
- Proof: Code: pool.rs:2345 `let claims = p.token_claims()?;` holds the claims THIS settlement found, and pool.rs:2356-2357 `let flushed = p.total_shares != 0 && ovf(p.token_claims()?.checked_mul(VEST_FLUSH_RATIO))? <= claims;`. The deposit_in gate (pool.rs:1971-1973) applies only while `p.borrowed_q != 0`. Trace on shortwin-math e035f2e, calling pool::* in the program's order: A deposits 1,000,000 raw. Two shorts borrow 450,000 each (90 %). A withdraws 99,500, its allowance: T = 500, claims 900,500. Settlement 1 (q 450,000, sol_in 50 SOL, lump 25 SOL): claims 900,500 -> 450,500; 450,500*1,000 > 900,500, so no flush. Settlement 2 (same): claims 450,500 -> 500; 500*1,000 = 500,000 > 450,500, so no flush (the epoch rebases 1/1,000). The pool now holds 0.056 % of its claims, below AIR-07's 1/1,000. J deposits 1,000,000 raw. PoolDrained does not apply because ΣB_Q = 0. J holds for lump_vest_s (21,600 s), is paid 49,974,990,303 lamports of the 50 SOL vest and withdraws 999,999 tokens. A is paid 50,025,009,695. Same run with ONE 900,000 short and a 50 SOL lump: claims 900,500 -> 500, the vest is flushed, A is paid 99,999,999,999 and J 0. A 500-raw deposit by J after the split takes 24,988,890,110. With two settlements the final claims can be about 1/10^6 of the original without any flush, because each settlement only has to leave just over 1/1,000 of what it found. Stated intent this breaks: the settle comment says 'the old shares' token claims are (nearly) wiped: they keep their unreleased lumps ... instead of leaving them to the next depositor'. ECON §9.5 says 'With the gate a just-in-time deposit around any settlement captures nothing'. Any market with two or more positions settling in one crash reaches this state with no attacker. An outsider only bundles the deposit.
- Fix: Test the AIR-07 flush and the PoolDrained gate against the highest token claims since the vest was last empty, not one settlement's claims.

```diff
--- a/crates/shortwin-math/src/pool.rs
+++ b/crates/shortwin-math/src/pool.rs
@@ pub struct Pool {
     pub wipe_swept: u8,
     /// Explicit padding (zero).
     pub _pad: [u8; 2],
+    /// AIR-07's reference: the highest token claims a settlement found since `vest_bal` was 0.
+    pub vest_ref: u64,
 }
@@ fn deposit_in(
     if p.total_shares != 0
         && p.borrowed_q != 0
-        && ovf(u128::from(p.tokens_free).checked_mul(VEST_FLUSH_RATIO))? <= p.token_claims()?
+        && ovf(u128::from(p.tokens_free).checked_mul(VEST_FLUSH_RATIO))?
+            <= p.token_claims()?.max(u128::from(p.vest_ref))
     {
@@ pub fn settle(
     let (mut p, mut v) = (*pool, *vault);
-    // AIR-07: the token claims the settlement finds.
-    let claims = p.token_claims()?;
+    // AIR-07: the claims the running vest was earned at, not only those this settlement finds.
+    if p.vest_bal == 0 {
+        p.vest_ref = 0;
+    }
+    let claims = p.token_claims()?.max(u128::from(p.vest_ref));
+    p.vest_ref = to_u64(claims)?;
@@
     if flushed {
         flush_vest_in(&mut p, now)?;
+        p.vest_ref = 0;
     }
(Market stores vest_ref in 8 bytes of _reserved; Market::pool/set_pool and the layout asserts follow.)
```

#### `shortwin-math|sol-settle|s3-wait-from-market-chain` — LEAD — `settlement:6-periphery`

- Location: `crates/shortwin-math/src/settle.rs:364`
- Description: sol_settle times a position's S3 from the market's chain, so any keeper settles a position at once when the fee first passes its higher cap. We did not measure how often two open positions lock different caps.
- Fix: Keep a per-cap or per-position no-route start and time each position's S3 from its own start.
- Code smells: settle::s3_due starts at chain.no_route_since, the market chain judged at chain_hard_cap_bps = min(global cap, Market.min_open_hard_cap_bps) (route.rs:43-49), while no_route_now is position_no_route at min(global cap, the position's own cap) (sol_settle.rs:242). Positions A (cap 300) and B (cap 500): a 4 % fee from t = 0 runs the chain for A; at t = 1,000 the fee passes 5 %, B is no route and s3_due(B) holds at once (0 + 600 <= 1,000), without B's own s3_wait_s.

#### `shortwin-math|touch-pot|hardening-chain-epoch-unchecked` — LEAD — `lending:10-numerical-gap`

- Location: `crates/shortwin-math/src/pool.rs:1637`
- Description: touch_pot treats every lender with a forward chain as held and never checks that it is behind, so a current chained lender keeps its chain. Today every path that makes a lender current also clears the chain.
- Seam: boundary x invariant: the current-epoch case of the forward chain
- Path: third party -> sync_lender (unsigned) -> touch_pot on a lender with fwd != FWD_NONE -> returns (Behind, held = true) without an epoch check -> fwd_walk and touch_tail(Behind, held) -> no current-epoch settlement of the chain, no expired-lock drop, no clear_fwd
- Proof: touch_pot (pool.rs:1637-1643): `if l.fwd == FWD_NONE { convert_steps(..) } else { (Standing::Behind, true) }` never compares l.epoch with pool.share_epoch. Take a lender with fwd = FWD_OPEN at the current epoch. fwd_walk accrues the chain to pool.acc, and touch_tail(Behind, held = true) only moves the surplus. It skips the Current branch that settles the chain, the expired-lock drop and clear_fwd, so closable (it needs fwd == FWD_NONE) stays false until the owner's own touch. Today no stored position reaches that state: sync_lender's owner path runs pool::touch whenever convert returns Current or Parked, and every other lender instruction's touch_tail(Current) clears the chain.
- Fix: Take the conversion path when l.epoch == pool.share_epoch, so touch_tail(Current) settles and clears the chain.
- Code smells: A state flag (fwd) stands in for the epoch comparison it implies today; nothing checks that implication.

```diff
--- a/crates/shortwin-math/src/pool.rs
+++ b/crates/shortwin-math/src/pool.rs
@@ -1637,1 +1637,3 @@ pub fn touch_pot(
-    let (st, held) = if l.fwd == FWD_NONE {
+    // A current lender is never held: its chain ends here, and touch_tail(Current) settles
+    // and clears it.
+    let (st, held) = if l.fwd == FWD_NONE || l.epoch == pool.share_epoch {
```

#### `shortwin-math|withdraw|stale-lock-era-b-min` — LEAD — `lending:11-trust-gap`

- Location: `crates/shortwin-math/src/pool.rs:1825`
- Description: A lender who withdraws while ΣB_Q is dust sets the era's b_min, so later locks stay until lock_until after their borrowers repay. KI-13 accepts this bound; we did not confirm that its decision priced a trigger that any lender can send on purpose.
- Seam: three-way (access × economics × asymmetry)
- Path: Attacker deposits. While no other position is open, it opens its own minimum short (open_short is permissionless) and withdraws a dust amount (shortwin::withdraw → pool::withdraw → withdraw_in). lock_norm == 0, so v.b_min = ΣB_Q = the dust short. Honest borrowers then open large shorts, and honest lenders withdraw their free part. Their locks inherit the dust b_min. When the large shorts repay, resolve_in sees bn = dust, which is not below b_min, so Π does not decay. The honest locks hold to lock_until, and the attacker, now the only unlocked holder, takes the repaid tokens.
- Proof: PoC: crates/shortwin-math copied to the scratchpad, <scratch> (M = 1e9). Two lenders deposit 100 M each. Attacker: borrow 2,000,000, then withdraw 1,000,000 tokens, which gives b_min = 2,000,000. An honest borrower takes 160e9. The honest lender withdraws 19,998,599,993, so its 80,001,400,007,000 remaining shares are fully locked (lock_until day 134). The 160e9 short repays on day 102: bn = 2,000,000 and `bn < v.b_min` is false, so Π stays 1.0000. Honest allowance = 0 on day 102 and still 0 on day 122. Attacker allowance = 99,999,000,000 (its whole value), and it withdraws it. Control dustera_control.rs (no dust withdrawal, so the honest lock opens the era at b_min = 160,002,000,000): the same repayment sets Π to 0.0125 and the honest allowance to 79,999,555,555 at once.
- Fix: Give each lock its own decay base, the ΣB_Q when the lock is set (AI review report-5 finding 4, option A), so a later lock decays when ΣB_Q falls below its own base.
- Code smells: withdraw_in sets `v.b_min = p.borrowed_q` and Π = WAD only when `v.lock_norm == 0`, so the first lock of an era fixes b_min for every later lock. resolve_in decays Π only when `bn < v.b_min`. The opener's dust lock and every later lock keep lock_norm above 0, so the era and its dust b_min can last for as long as locks overlap. The opener's dust short can also carry the maximum duration, which pushes max_release and every later lock_until about 94 days out. KI-13 and AIR-11 (owner decision 7, 2026-10-12) accept this over-locking within lock_until (about 31 d by default, about 101 d at most). The register does not name a lender that triggers it on purpose for the cost of a minimum short (min_notional 0.053 SOL).

#### `shortwin|add-margin|hardening-zero-valued-tag` — LEAD — `positions:9-account-validation`

- Location: `programs/shortwin/src/state/position.rs:19`
- Description: add_margin treats status 0 as an open position, and 0 is also the value of zeroed data. A future exit that zeroes a position keeps it open to margin.
- Fix: Give the open status a value other than 0 in the next ShortPosition layout version, and keep 0 for an invalid or closed position.
- Code smells: `pub const POSITION_OPEN: u8 = 0;` is the only status any instruction writes, and add_margin.rs:65 gates on `position.status == POSITION_OPEN`. Zeroed or freshly allocated status bytes therefore read as open. Today Anchor's discriminator check and the exits' account close keep zeroed data out.

#### `shortwin|close-short|carried-position-stale-ert` — LEAD — `positions:1-math-precision`

- Location: `programs/shortwin/src/instructions/close_short.rs:192`
- Description: close_short reserves the curve-era ert of a position carried onto Raydium, so the borrower needs up to 0.01 SOL more to close.
- Path: programs/shortwin/src/instructions/close_short.rs
- Proof: A Tier D curve position (pos.ert = 6,500,000) is carried onto AMM v4 with V = b_hat + r_due + 6,000,000. close_short computes need = b_hat + r_due + 6,500,000 > V and reverts InsufficientVault, although direct_buy can never spend more than max_in = b_hat. The borrower must add_margin at least 500,000 lamports (returned at close) or wait for a keeper exit. Unverified: whether A26 means the open venue's ert to survive a pool switch.
- Fix: Reserve only the exit venue's own tolerance: use venues::effective_ert(kind, pos.ert), which is 0 on Raydium and the mock, in need, close_max_in, the leg and the spend bound.
- Code smells: open_short locks pos.ert = effective_ert(PumpCurve, exec_rent_tol): 6,500,000 lamports by default, 10,000,000 at the bound. A curve that graduates elsewhere moves to None, and a collection window carries its open positions onto a Raydium pool with every locked term (engine::pool_select, M4-SEC-4). close_short then requires b_hat + r_due + pos.ert <= pos.v (lines 192-196), passes pos.ert to close_max_in (line 197) and to the leg (line 268), and allows max_in + pos.ert as the spend bound (line 277). raydium::direct_buy funds trader_wsol with max_in only and requires spent <= max_in, so Raydium takes no venue-side rent. The ShortPosition.ert doc says '0 on Raydium'. venue_at_open is stored but never read. The listed max_in_liq lead has the same root cause; the AI review's report-3 leads list this close_short variant, never triaged.

```diff
--- a/programs/shortwin/src/instructions/close_short.rs
+++ b/programs/shortwin/src/instructions/close_short.rs
@@
             .ok_or(ShortwinError::InsufficientVault)?
             .cost;
+        // A curve position carried onto Raydium keeps its locked `ert`, but Raydium takes no
+        // venue-side rent: reserve only the exit venue's own tolerance.
+        let ert = venues::effective_ert(kind, pos.ert);
         let need = b_hat
             .checked_add(r_due)
-            .and_then(|x| x.checked_add(pos.ert))
+            .and_then(|x| x.checked_add(ert))
             .or_math()?;
         require_gte!(pos.v, need, ShortwinError::InsufficientVault);
-        let max_in = bounds::close_max_in(pos.v, r_due, pos.ert, max_buyback_cost)?
+        let max_in = bounds::close_max_in(pos.v, r_due, ert, max_buyback_cost)?
             .ok_or(ShortwinError::InsufficientVault)?;
@@
-        ert: pos.ert,
+        ert: venues::effective_ert(kind, pos.ert),
@@
-    let bound = max_in.checked_add(pos.ert).or_math()?;
+    let bound = max_in
+        .checked_add(venues::effective_ert(kind, pos.ert))
+        .or_math()?;
```

#### `shortwin|close-short|carried-position-stale-ert` — LEAD — `positions:6-periphery`

- Location: `programs/shortwin/src/instructions/close_short.rs:192-198`
- Description: close_short reserves the position's pump ert on a Raydium market, so a borrower near its margin edge cannot close though the leg spends no ert. We did not measure how often a carried position reaches that edge.
- Path: programs/shortwin/src/instructions/close_short.rs
- Fix: Reserve `venues::effective_ert(kind, pos.ert)` of the current venue in close_short, as the leg needs.
- Code smells: `need = b_hat + r_due + pos.ert` and `bounds::close_max_in(pos.v, r_due, pos.ert, max_buyback_cost)` use the ert locked at open (`effective_ert(kind_at_open, exec_rent_tol)`, 6,500,000 lamports by default on the pump venues); a curve market that graduates elsewhere carries its positions to None and then to Raydium through the collection window; raydium::direct_buy spends no venue-side rent; the ShortPosition.ert doc says "0 on Raydium"; same root cause as the known max_in_liq lead.

#### `shortwin|close-short|hardening-deny-list-gap` — LEAD — `positions:8-asymmetry`

- Location: `programs/shortwin/src/instructions/close_short.rs:148`
- Description: The close_short deny list omits trader_base_ata and ATA(trader, WSOL), so an extras set from the Emergency multisig can name them in the venue CPI. Only that multisig writes extras; we found no path for any other caller.
- Path: An extras set from the Emergency multisig that names trader_base_ata or ATA(trader, WSOL) (writable) passes parse_extras on close_short, and raydium::invoke_as_trader appends it to the venue CPI, which the trader PDA signs.
- Proof: close_short.rs:148-158 `let deny = [config, market_key, market.market_vault, market.pool_vault, market.trader, market.trader_wsol, position_key, exposure, borrower_key]` - no market.trader_base_ata and no pump_curve::trader_wsol_ata(&market). parse_extras rejects only signers, short.win-owned accounts and deny/pool_deny keys (venues/mod.rs:212-216). Both omitted accounts are owned by the token program and pass. keeper_exit (liquidate.rs:533-543) has the same list.
- Fix: Add market.trader_base_ata and the cached ATA(trader, WSOL) to the deny lists of close_short and keeper_exit.
- Code smells: This is the sibling instance of the known open_short hardening-deny-list-gap lead. The keeper exits (liquidate, expire) share the gap through keeper_exit.

```diff
--- a/programs/shortwin/src/instructions/close_short.rs
+++ b/programs/shortwin/src/instructions/close_short.rs
@@
             market.trader,
             market.trader_wsol,
+            market.trader_base_ata,
+            venues::pump_curve::trader_wsol_ata(&market),
             position_key,
```

#### `shortwin|cover-deficit|hardening-clock-not-clamped` — LEAD — `lending:10-numerical-gap`

- Location: `programs/shortwin/src/instructions/cover_deficit.rs:61`
- Description: cover_deficit runs the lender touch at the raw clock, not market_now. A clock step back of more than lock_grace_s then makes a withdraw that adds a lock fail.
- Seam: boundary x invariant: the lock calendar uses the raw clock in one lender path and market_now in the others
- Path: owner -> cover_deficit(lamports) -> handler: now = clock.unix_timestamp (cover_deficit.rs:61) -> pool::cover(&pool_state, &mut vault, &mut l, &mut records, now, lamports) (cover_deficit.rs:73) -> touch_in: cal_advance(vault, now) moves MarketVault.cal_day to floor(now/86,400) + 1, and touch_tail drops an expired lock at the raw clock -> later deposit / withdraw / exits run at prologue::market_now = max(clock, last_accrual_ts), which cover_deficit never raises
- Proof: cover_deficit.rs:61 `let now = clock.unix_timestamp;`, while deposit.rs:119 and withdraw.rs:175 use `prologue::market_now(&market, clock.unix_timestamp)`; the prologue module docs say every step runs at market_now. cal_advance returns early when today < cal_day, so cal_day never moves back. Trace with a clock step back: cover_deficit at t1 = 86,400*D + 10 sets cal_day = D + 1. A later withdraw that adds a lock, at market_now t2 = 86,400*D - 3,600 (clock 3,610 s behind, last_accrual_ts < t2, lock_grace_s = 3,600, max_release <= t2), computes lock_until_for = ceil((t2 + 3,600)/86,400)*86,400 = 86,400*D, which is day D < cal_day. cal_add returns InvalidInput at pool.rs:833, so the withdraw fails until the clock catches up. Unproven: a step back of about one hour on mainnet. This is the third instance of the pattern recorded for claim_sol and sync_lender.
- Fix: Run the touch at prologue::market_now(&market, clock.unix_timestamp), as deposit and withdraw do.
- Code smells: The handler passes raw `clock.unix_timestamp` to `pool::cover`, and that call runs `cal_advance` and the lender's lock expiry. deposit and withdraw clamp the same time with `market_now`. The same raw clock is already recorded for claim_sol (`hardening-clock-not-clamped`) and sync_lender (`hardening-raw-clock-lock-calendar`).

```diff
--- a/programs/shortwin/src/instructions/cover_deficit.rs
+++ b/programs/shortwin/src/instructions/cover_deficit.rs
@@ -58,5 +58,9 @@ pub fn handler<'info>(ctx: Context<'info, CoverDeficit<'info>>, lamports: u64) -> Result<()> {
     require!(lamports > 0, ShortwinError::ZeroAmount);
     let clock = Clock::get()?;
-    let now = clock.unix_timestamp;
+    // Never before the market's last accrual, as in deposit and withdraw.
+    let now = {
+        let market = ctx.accounts.market.load()?;
+        crate::engine::prologue::market_now(&market, clock.unix_timestamp)
+    };
     let market_key = ctx.accounts.market.key();
```

#### `shortwin|cover-deficit|hardening-clock-not-clamped` — LEAD — `lending:11-trust-gap`

- Location: `programs/shortwin/src/instructions/cover_deficit.rs:61`
- Description: cover_deficit runs the lender touch at the raw clock, not at market_now as deposit and withdraw do. We found no wrong value today; a time-dependent read added there runs before the last accrual.
- Path: Owner-signed cover_deficit → pool::cover → touch_in, which runs cal_advance(v, now) and the own-lock clear at the raw Clock::unix_timestamp.
- Proof: cover_deficit.rs:61 `let now = clock.unix_timestamp;` and :73 `pool::cover(&pool_state, &mut vault, &mut l, &mut records, now, lamports)`. deposit.rs:119 and withdraw.rs:175 use `prologue::market_now(&market, clock.unix_timestamp)`, which is never before Market.last_accrual_ts.
- Fix: Use prologue::market_now(&market, clock.unix_timestamp) as now, as deposit and withdraw do.
- Code smells: This is a repeat of claim_sol's known lead, in an instruction that the known list does not name. The raw clock is never later than market_now, cal_advance ignores a day before cal_day, and the lock clear only waits, so no value is wrong today. A vest release or an allowance read added to this market-read-only path gets InvalidInput (now < vest_last) or a lock that is still live after a clock step back.

```diff
--- a/programs/shortwin/src/instructions/cover_deficit.rs
+++ b/programs/shortwin/src/instructions/cover_deficit.rs
@@ -59,7 +59,10 @@ pub fn handler<'info>(ctx: Context<'info, CoverDeficit<'info>>, lamports: u64) -> Result<()> {
     let clock = Clock::get()?;
-    let now = clock.unix_timestamp;
+    let now = crate::engine::prologue::market_now(
+        &*ctx.accounts.market.load()?,
+        clock.unix_timestamp,
+    );
     let market_key = ctx.accounts.market.key();
```

#### `shortwin|cover-deficit|hardening-clock-not-clamped` — LEAD — `lending:2-access-control`

- Location: `programs/shortwin/src/instructions/cover_deficit.rs:61`
- Description: cover_deficit runs the lender touch at the raw clock, but deposit and withdraw run it at market_now. We found no path today where the two times differ.
- Path: programs/shortwin/src/instructions/cover_deficit.rs
- Proof: cover_deficit.rs:61 `let now = clock.unix_timestamp;` vs prologue.rs:55-57 `pub fn market_now(market: &Market, clock_now: i64) -> i64 { clock_now.max(market.last_accrual_ts) }`.
- Fix: Compute now with prologue::market_now from the loaded market in cover_deficit, as deposit and withdraw do.
- Code smells: `let now = clock.unix_timestamp;` (cover_deficit.rs:61) feeds pool::cover -> touch_in -> cal_advance(v, now) and the lock-expiry clear in touch_tail. deposit.rs:119 and withdraw.rs:175 use `prologue::market_now(&market, clock.unix_timestamp)` = `clock_now.max(market.last_accrual_ts)`. Today last_accrual_ts is written only from market_now (prologue.rs:107, create_market.rs:655) and the Clock is monotonic, so the values agree. A future write of last_accrual_ts ahead of the clock would make this instruction drop calendar buckets and clear locks on a different time base than the market. Same class as the recorded claim_sol lead.

```diff
--- a/programs/shortwin/src/instructions/cover_deficit.rs
+++ b/programs/shortwin/src/instructions/cover_deficit.rs
@@
     let clock = Clock::get()?;
-    let now = clock.unix_timestamp;
+    let now = crate::engine::prologue::market_now(
+        &*ctx.accounts.market.load()?,
+        clock.unix_timestamp,
+    );
     let market_key = ctx.accounts.market.key();
```

#### `shortwin|cover-deficit|hardening-clock-not-clamped` — LEAD — `lending:3-economic-security`

- Location: `programs/shortwin/src/instructions/cover_deficit.rs:61`
- Description: cover_deficit runs the lender touch at the raw clock, not at market_now, so it can advance the lock calendar on another time base.
- Path: programs/shortwin/src/instructions/cover_deficit.rs
- Fix: Use prologue::market_now(&market, clock.unix_timestamp) as now, as deposit and withdraw do.
- Code smells: cover_deficit.rs:61 uses `let now = clock.unix_timestamp;` for pool::cover, whose touch runs cal_advance, writes the result to MarketVault and checks the lender's own lock expiry. This is the same root cause as the claim_sol lead, in a function the earlier scans do not list.

```diff
--- a/programs/shortwin/src/instructions/cover_deficit.rs
+++ b/programs/shortwin/src/instructions/cover_deficit.rs
@@ pub fn handler
-    let now = clock.unix_timestamp;
+    let now = crate::engine::prologue::market_now(&*ctx.accounts.market.load()?, clock.unix_timestamp);
```

#### `shortwin|cover-deficit|hardening-clock-not-clamped` — LEAD — `lending:4-execution-trace`

- Location: `programs/shortwin/src/instructions/cover_deficit.rs:61`
- Description: cover_deficit runs the lender touch at the raw clock, not market_now, so the lock calendar can move ahead of the market's own time. No path moves value today; a later reader that assumes market time would read a skewed calendar.
- Path: programs/shortwin/src/instructions/cover_deficit.rs
- Code smells: `let now = clock.unix_timestamp;` is passed to `pool::cover` (line 73), whose touch runs cal_advance and the lock-expiry clear. deposit and withdraw use `prologue::market_now` (max(clock, last_accrual_ts)). This is the same pattern as the known claim_sol and sync_lender leads.

#### `shortwin|cover-deficit|hardening-clock-not-clamped` — LEAD — `lending:5-invariant`

- Location: `programs/shortwin/src/instructions/cover_deficit.rs:61`
- Description: cover_deficit runs the lender touch at the raw clock, not at market_now, unlike deposit and withdraw. A later rule that reads the lock fields there sees an earlier time.
- Path: programs/shortwin/src/instructions/cover_deficit.rs
- Proof: cover_deficit.rs:61 `let now = clock.unix_timestamp;`; :73 `pool::cover(..., now, lamports)`; pool.rs cover -> touch_in -> cal_advance(v, now) and the touch_tail lock-expiry branch; prologue.rs:55 `pub fn market_now(market, clock_now) -> i64 { clock_now.max(market.last_accrual_ts) }`, used by deposit.rs and withdraw.rs.
- Fix: Run the touch at prologue::market_now(&market, clock.unix_timestamp), as deposit and withdraw do.
- Code smells: `let now = clock.unix_timestamp;` (line 61) goes straight into `pool::cover(&pool_state, &mut vault, &mut l, &mut records, now, lamports)` (line 73). pool::cover runs touch_in, which calls `cal_advance(v, now)` and the lender's own lock-expiry test `if l.lock != 0 && now >= l.lock_until && l.lock_epoch == v.pi_epoch`. deposit, withdraw and sweep_trader use `prologue::market_now(&market, clock.unix_timestamp)` = `clock_now.max(market.last_accrual_ts)`, and the prologue's contract says every step runs at market_now, never before the market's last accrual. Trace: a withdraw at last_accrual_ts = D*86400 treats a lock with lock_until = D*86400 as expired (lock_of returns 0). A cover_deficit at a clock that stepped back to D*86400 - 5 keeps l.lock non-zero and writes it back. No value moves today. This is the same class as the known claim_sol and sync_lender leads; it is missing from the known list for this instruction.

```diff
--- a/programs/shortwin/src/instructions/cover_deficit.rs
+++ b/programs/shortwin/src/instructions/cover_deficit.rs
@@
         let mut l = position.lender();
-        pool::cover(&pool_state, &mut vault, &mut l, &mut records, now, lamports).math()?;
+        let touch_now = crate::engine::prologue::market_now(&market, now);
+        pool::cover(&pool_state, &mut vault, &mut l, &mut records, touch_now, lamports).math()?;
```

#### `shortwin|cover-deficit|hardening-clock-not-clamped` — LEAD — `lending:6-periphery`

- Location: `programs/shortwin/src/instructions/cover_deficit.rs:61`
- Description: cover_deficit touches the lender at the raw clock, not `market_now`, so after a clock step back it reads locks earlier than withdraw does. No caller gains value today.
- Path: Clock step back -> cover_deficit's pool::cover touch runs cal_advance and the lock-expiry clear at the raw clock, while deposit/withdraw use market_now.
- Fix: Compute `now` with `prologue::market_now` from the loaded market.
- Code smells: `let now = clock.unix_timestamp;` feeds pool::cover (touch_in: cal_advance, lock clear); the same class as claim_sol.

```diff
--- a/programs/shortwin/src/instructions/cover_deficit.rs
+++ b/programs/shortwin/src/instructions/cover_deficit.rs
@@ -61,1 +61,2 @@
-    let now = clock.unix_timestamp;
+    let now =
+        crate::engine::prologue::market_now(&*ctx.accounts.market.load()?, clock.unix_timestamp);
```

#### `shortwin|cover-deficit|hardening-clock-not-clamped` — LEAD — `lending:7-first-principles`

- Location: `programs/shortwin/src/instructions/cover_deficit.rs:61`
- Description: cover_deficit runs the lender touch and the calendar advance at the raw clock, not at market_now. After a clock step back, a later withdraw at market_now can fail in cal_add.
- Path: programs/shortwin/src/instructions/cover_deficit.rs
- Code smells: `let now = clock.unix_timestamp;` goes to pool::cover, whose touch_in runs cal_advance and the expired-lock drop at that time; Market stays read-only, so last_accrual_ts does not move; this is the same pattern as claim_sol.rs:76 and sync_lender.rs:104, while deposit and withdraw clamp with prologue::market_now.

#### `shortwin|cover-deficit|hardening-clock-not-clamped` — LEAD — `lending:8-asymmetry`

- Location: `programs/shortwin/src/instructions/cover_deficit.rs:61`
- Description: cover_deficit runs the lender touch at the raw clock, so it can move the lock calendar past the time that later market instructions use. We did not prove a loss.
- Path: programs/shortwin/src/instructions/cover_deficit.rs
- Proof: `let now = clock.unix_timestamp;` goes to pool::cover, then touch_in, which runs cal_advance(v, now) and the lock-expiry clear in touch_tail. deposit, withdraw and sweep_trader use prologue::market_now = max(clock, last_accrual_ts). cover_deficit does not write Market.last_accrual_ts, and MarketVault keeps no time of its own. If the clock steps back across a day end, a later withdraw at market_now < this now sees a lender's lock as live (lock_of) after the calendar has already dropped its bucket. The same class is known on claim_sol and sync_lender; this is a third instance.
- Fix: Run the touch at max(clock, market.last_accrual_ts), the time market_now gives, as deposit and withdraw do.
- Code smells: Read-only-Market instructions (claim_sol, cover_deficit, sync_lender) take a different time base from the Market-writing lender instructions while they write the same MarketVault calendar.

```diff
--- a/programs/shortwin/src/instructions/cover_deficit.rs
+++ b/programs/shortwin/src/instructions/cover_deficit.rs
-    let now = clock.unix_timestamp;
+    let now = crate::engine::prologue::market_now(&*ctx.accounts.market.load()?, clock.unix_timestamp);
```

#### `shortwin|cover-deficit|hardening-clock-not-clamped` — LEAD — `lending:9-account-validation`

- Location: `programs/shortwin/src/instructions/cover_deficit.rs:61`
- Description: cover_deficit runs the lender touch at the raw clock, not at market_now, so its lock calendar time can differ from deposit and withdraw. We found no harmful path.
- Path: programs/shortwin/src/instructions/cover_deficit.rs
- Fix: Run the touch at prologue::market_now of the read-only market, as deposit and withdraw do.
- Code smells: cover_deficit.rs:61 sets now = clock.unix_timestamp, and pool::cover at :73 passes it to cal_advance and the lock-expiry clear. deposit.rs and withdraw.rs use prologue::market_now, which is max(clock, last_accrual_ts). claim_sol has the same pattern (listed); this is the same class in a new instruction.

```diff
--- a/programs/shortwin/src/instructions/cover_deficit.rs
+++ b/programs/shortwin/src/instructions/cover_deficit.rs
-    let now = clock.unix_timestamp;
+    let now = crate::engine::prologue::market_now(
+        &*ctx.accounts.market.load()?,
+        clock.unix_timestamp,
+    );
```

#### `shortwin|deposit|hardening-version-unchecked` — LEAD — `lending:9-account-validation`

- Location: `programs/shortwin/src/state/lender.rs:26`
- Description: LenderPosition, ShareEpoch and MarketVault carry no layout version, so a later layout change makes every lender instruction read old accounts with the new layout.
- Path: programs/shortwin/src/state/lender.rs
- Fix: Add a version byte to each lender account and check it in every loader.
- Code smells: Market and GlobalConfig carry a version that each Accounts struct checks. LenderPosition (lender.rs:26, KI-66 used 22 of the reserved bytes without a migration), ShareEpoch (epoch.rs:24, written in place at offset 127) and MarketVault (vault.rs:26, zero-copy) have no version field. load_records, AccountLoader<MarketVault> and Account<LenderPosition> check only the discriminator and the PDA.

#### `shortwin|expire|hardening-event-destination-hardcoded` — LEAD — `positions:4-execution-trace`

- Location: `programs/shortwin/src/instructions/expire.rs:167`
- Description: The expire instruction reports swept trader SOL as vested to lenders, but with no shares the pool books that SOL to the protocol.
- Path: programs/shortwin/src/instructions/expire.rs
- Proof: expire.rs:160-169: `if out.swept.any() { emit_cpi!(TraderSwept { .. to: SWEEP_TO_VEST, wipe_bits: 0 }) }` while the booking in keeper_exit (liquidate.rs:612-623) credits protocol_fees when total_shares == 0 (pool.rs credit_in).
- Fix: Emit the destination that keeper_exit derives from the pool's total_shares, as sweep_trader does.
- Code smells: expire shares keeper_exit with liquidate, so the normalisation's lamports go through pool::sweep(None, SweepSource::Trader), and to protocol_fees while total_shares == 0. The handler emits TraderSwept { to: SWEEP_TO_VEST } unconditionally (expire.rs:167). A dust position left after the last lender's exit makes S = 0 with a position still open (pool.rs accrue doc; AIR-08).

```diff
--- a/programs/shortwin/src/instructions/expire.rs
+++ b/programs/shortwin/src/instructions/expire.rs
@@
-            to: SWEEP_TO_VEST,
+            to: out.sweep_to,
```

#### `shortwin|expire|hardening-event-destination-hardcoded` — LEAD — `positions:7-first-principles`

- Location: `programs/shortwin/src/instructions/expire.rs:167`
- Description: expire always emits TraderSwept with to SWEEP_TO_VEST and wipe_bits 0. The value is correct only while withdraw keeps S above 0 with a position open.
- Path: programs/shortwin/src/instructions/expire.rs
- Proof: expire.rs:161-169 has `to: SWEEP_TO_VEST, wipe_bits: 0`.
- Fix: Return the sweep destination and the wipe bits from keeper_exit and put them in the event.
- Code smells: expire shares keeper_exit with liquidate. The event constants at expire.rs:167-168 do not reflect how pool::sweep routed the SOL.

#### `shortwin|expire|hardening-event-destination-hardcoded` — LEAD — `settlement:4-execution-trace`

- Location: `programs/shortwin/src/instructions/expire.rs:167`
- Description: expire reports every trader sweep as SWEEP_TO_VEST with wipe_bits 0, although pool::sweep can send it to protocol_fees and the market keeps other bits.
- Fix: Emit the destination and wipe bits that keeper_exit returns, as sweep_trader does.
- Code smells: expire.rs:167-168 hard-codes `to: SWEEP_TO_VEST, wipe_bits: 0` after the shared keeper_exit normalisation. The same pool::sweep paths apply: protocol_fees with S = 0, the trader bit set, and wipe_swept = 3 on every market that never wiped (pool.rs:400, 2174-2177).

#### `shortwin|expire|hardening-event-destination-hardcoded` — LEAD — `settlement:7-first-principles`

- Location: `programs/shortwin/src/instructions/expire.rs:167`
- Description: expire reports every trader sweep as SWEEP_TO_VEST with wipe_bits 0, but pool::sweep sends it to protocol_fees and sets a wipe bit when S is 0. An indexer that books from the event then shows a wrong vest and a wrong wipe gate.
- Path: programs/shortwin/src/instructions/expire.rs
- Fix: Compute `to` and `wipe_bits` from the pool after pool::sweep, as sweep_trader does.
- Code smells: Same hardcoded TraderSwept { to: SWEEP_TO_VEST, wipe_bits: 0 } as liquidate; the shared keeper_exit books the sweep with pool::sweep, which goes to protocol_fees and sets WIPE_TRADER_SWEPT when total_shares == 0.

#### `shortwin|expire|hardening-event-destination-hardcoded` — LEAD — `settlement:8-asymmetry`

- Location: `programs/shortwin/src/instructions/expire.rs:167`
- Description: expire reports every trader sweep as SWEEP_TO_VEST. If S could reach 0 with a position open, the event would name the wrong destination.
- Code smells: expire.rs:167 always emits `to: SWEEP_TO_VEST` from keeper_exit's normalisation, as liquidate and close_short do. pool::sweep credits the protocol when S = 0, which LastSharesBorrowed prevents today.

#### `shortwin|liquidate|hardening-asymmetric-zero-price-sigma` — LEAD — `settlement:8-asymmetry`

- Location: `programs/shortwin/src/instructions/liquidate.rs:344`
- Description: assess reverts on a zero live price, while sol_settle reads the same state as infinite slippage. Only a pool with almost no SOL reaches this branch.
- Code smells: liquidate.rs:344-349 calls risk::sigma_xy on the live state. A zero price (y*WAD < x) gives DivByZero, and `.math()?` reverts. arming.rs:101-109 maps the same state to sigma = u64::MAX. Only a pool with fewer than about 19 lamports of SOL reserve reaches it, and such a pool is not executable today.

```diff
--- a/programs/shortwin/src/instructions/liquidate.rs
+++ b/programs/shortwin/src/instructions/liquidate.rs
@@ -344,6 +344,9 @@ pub(crate) fn assess(
     let sigma_live = match b_live {
-        Some(b) if state.x > 0 => {
-            risk::sigma_xy(Some(b.net), q, u128::from(state.x), u128::from(state.y)).math()?
-        }
+        Some(b) if state.x > 0 && state.y > 0 => {
+            match risk::sigma_xy(Some(b.net), q, u128::from(state.x), u128::from(state.y)) {
+                Err(shortwin_math::MathError::DivByZero) => u64::MAX,
+                r => r.math()?,
+            }
+        }
         _ => u64::MAX,
     };
```

#### `shortwin|liquidate|hardening-deny-list-gap` — LEAD — `settlement:6-periphery`

- Location: `programs/shortwin/src/instructions/liquidate.rs:533`
- Description: The keeper_exit deny list omits trader_base_ata and ATA(trader, WSOL), so an Emergency extras set can pass them to the venue CPI that the trader signs. We did not find a venue that misuses such a trailing account today.
- Fix: Add market.trader_base_ata and market.trader_wsol_ata to the keeper_exit deny list.
- Code smells: keeper_exit (shared by liquidate and expire) builds deny = [config, market, market_vault, pool_vault, trader, trader_wsol, position, exposure, borrower] (liquidate.rs:533-543). market.trader_base_ata and market.trader_wsol_ata (= ATA(trader, WSOL)) are token accounts of the trader PDA: not signers, not short.win-owned, not in pool_denylist (PumpCurve: bonding_curve, curve_base_ata, curve_quote_ata), so parse_extras accepts them; raydium::invoke_as_trader appends the extras to a CPI signed with the trader seeds. Same class as open_short's hardening-deny-list-gap and parse_extras' hardening-extras-deny-trader-base-ata.

#### `shortwin|liquidate|hardening-event-destination-hardcoded` — LEAD — `positions:4-execution-trace`

- Location: `programs/shortwin/src/instructions/liquidate.rs:206`
- Description: The liquidate instruction reports swept trader SOL as vested to lenders, but with no shares the pool books that SOL to the protocol.
- Path: programs/shortwin/src/instructions/liquidate.rs
- Proof: liquidate.rs:206 `to: SWEEP_TO_VEST,` with no total_shares test; pool.rs vest_in: `if p.total_shares == 0 { return credit_in(p, v, wipe, i128::from(lamports), true); }` -> credit_in with wipe None and !is_wipe_live(): `v.lender_sol = sub(..); v.protocol_fees = add(..)`. An indexer that trusts the event credits lenders with SOL the program gave to the protocol. No funds move wrongly: this is event accuracy only.
- Fix: Return the sweep destination from keeper_exit (SWEEP_TO_PROTOCOL when the pool has no shares, else SWEEP_TO_VEST) and emit that value.
- Code smells: keeper_exit books the normalisation's lamports with pool::sweep(&mut pool, &mut vault, None, now, lump_vest_s, swept.lamports, SweepSource::Trader) (liquidate.rs:612-623). With total_shares == 0, vest_in calls credit_in(.., None, ..), which moves the amount from lender_sol to protocol_fees (pool.rs credit_in, R3A-10). The handler still emits TraderSwept { to: SWEEP_TO_VEST } unconditionally (liquidate.rs:206). sweep_trader derives `to` from the pool state (sweep_trader.rs:313-320). S = 0 with an open position is reachable: pool.rs accrue says a credit while S = 0 is 'only possible with a dust position left after the last lender', and withdraw clears wipe_swept when total_shares reaches 0 (AIR-08). Same class as the listed close_short lead; expire has the same line.

```diff
--- a/programs/shortwin/src/instructions/liquidate.rs
+++ b/programs/shortwin/src/instructions/liquidate.rs
@@ keeper_exit
     let swept = trader.normalise()?;
+    let sweep_to = if pool.total_shares > 0 { SWEEP_TO_VEST } else { SWEEP_TO_PROTOCOL };
@@ handler
-            to: SWEEP_TO_VEST,
+            to: out.sweep_to,
```

#### `shortwin|liquidate|hardening-event-destination-hardcoded` — LEAD — `positions:7-first-principles`

- Location: `programs/shortwin/src/instructions/liquidate.rs:206`
- Description: liquidate always emits TraderSwept with to SWEEP_TO_VEST and wipe_bits 0. The value is correct only while withdraw keeps S above 0 with a position open.
- Path: programs/shortwin/src/instructions/liquidate.rs
- Proof: liquidate.rs:200-208 has `to: SWEEP_TO_VEST, wipe_bits: 0`. The routing is in pool.rs `sweep` and `vest_in`.
- Fix: Return the sweep destination and the wipe bits from keeper_exit and put them in the event.
- Code smells: This is the same pattern as close_short. keeper_exit calls `pool::sweep(.., None, .., SweepSource::Trader)`, which routes by total_shares, but the handler emits constants.

#### `shortwin|liquidate|hardening-event-destination-hardcoded` — LEAD — `settlement:4-execution-trace`

- Location: `programs/shortwin/src/instructions/liquidate.rs:206`
- Description: liquidate reports every trader sweep as SWEEP_TO_VEST with wipe_bits 0, although pool::sweep can send it to protocol_fees and the market keeps other bits.
- Fix: Return the real destination and pool.wipe_swept from keeper_exit and emit them, as sweep_trader does.
- Code smells: liquidate.rs:206-207 hard-codes `to: SWEEP_TO_VEST, wipe_bits: 0`. events.rs documents wipe_bits as the Market.flags wipe bits after the sweep. Pool::new starts every market with wipe_swept = 3 (pool.rs:400), so the event is wrong on every market that never wiped. With S = 0 (a dust position after the last lender), keeper_exit's pool::sweep sends the SOL to protocol_fees and sets the trader bit (pool.rs:2174-2177). sweep_trader reports the real values (sweep_trader.rs:302-319, 400). An indexer that trusts these fields books protocol SOL as a lender vest.

```diff
--- a/programs/shortwin/src/instructions/liquidate.rs
+++ b/programs/shortwin/src/instructions/liquidate.rs
@@ keeper_exit
     let swept = trader.normalise()?;
+    let mut sweep_to = SWEEP_TO_VEST;
     if swept.lamports > 0 {
+        if pool.total_shares == 0 {
+            sweep_to = if pool.is_wipe_live() { SWEEP_TO_WIPE_RECORD } else { SWEEP_TO_PROTOCOL };
+        }
@@ handler
-            to: SWEEP_TO_VEST,
-            wipe_bits: 0,
+            to: out.sweep_to,
+            wipe_bits: out.wipe_bits,
```

#### `shortwin|liquidate|hardening-event-destination-hardcoded` — LEAD — `settlement:7-first-principles`

- Location: `programs/shortwin/src/instructions/liquidate.rs:206`
- Description: liquidate reports every trader sweep as SWEEP_TO_VEST with wipe_bits 0, but pool::sweep sends it to protocol_fees and sets a wipe bit when S is 0. An indexer that books from the event then shows a wrong vest and a wrong wipe gate.
- Path: programs/shortwin/src/instructions/liquidate.rs
- Fix: Compute `to` and `wipe_bits` from the pool after pool::sweep, as sweep_trader does.
- Code smells: emit_cpi!(TraderSwept { .., to: SWEEP_TO_VEST, wipe_bits: 0 }) is hardcoded. keeper_exit books the swept lamports with pool::sweep(.., None, ..), which credits protocol_fees and sets WIPE_TRADER_SWEPT when total_shares == 0 (a dust position after the last lender, ARCH §9 M0b note 4). sweep_trader.rs:313-319 computes `to` from the pool. events.rs documents wipe_bits as 'Market.flags wipe bits after the sweep'.

#### `shortwin|liquidate|hardening-event-destination-hardcoded` — LEAD — `settlement:8-asymmetry`

- Location: `programs/shortwin/src/instructions/liquidate.rs:206`
- Description: liquidate reports every trader sweep as SWEEP_TO_VEST. If S could reach 0 with a position open, the event would name the wrong destination.
- Code smells: liquidate.rs:206 always emits `to: SWEEP_TO_VEST`. pool::sweep sends the SOL to protocol_fees when S = 0 (pool.rs:2174 to credit_in). This is right today only because withdraw's LastSharesBorrowed keeps S > 0 while a position is open. This repeats the known close_short lead.

#### `shortwin|liquidate|hardening-pre-roll-ema-fee-cap` — LEAD — `settlement:4-execution-trace`

- Location: `programs/shortwin/src/venues/pump_amm.rs:715`
- Description: The keeper exits and sol_settle take the AIR-10 fee cap at the stored pre-roll P_ema, while every exit decision reads the post-roll P_ema.
- Fix: Resolve fee_ppm_at_ema again after prologue::run from the post-roll P_ema, on the same live reserves.
- Code smells: keeper_exit calls read_state_boxed_for_exit (liquidate.rs:555) before prologue::run (liquidate.rs:563), and sol_settle does the same (sol_settle.rs:205 vs 221). pump_amm::read_state resolves fee_ppm_at_ema from `market.ema_price.get()`, which is the stored pre-roll value (pump_amm.rs:715, 731). exit_risk_cap_bps then caps assess (liquidate.rs:338), B_ref (bounds.rs:114) and sol_settle's risk_quoter, while every exit predicate reads the post-roll observation. The two values differ only when a pump-fees tier boundary lies between them. The difference lasts one second, and we found no path that steers it.

#### `shortwin|liquidate|hardening-sigma-zero-price-revert` — LEAD — `settlement:6-periphery`

- Location: `programs/shortwin/src/instructions/liquidate.rs:346`
- Description: assess makes liquidate and expire fail with a math error when the live price rounds to 0, while sol_settle reads that state as infinite slippage. This needs an executable pool with a SOL reserve under 19 lamports, which we did not find.
- Fix: Map DivByZero from risk::sigma_xy to u64::MAX in assess, as RiskQuotes::sigma_of does.
- Code smells: assess: `Some(b) if state.x > 0 => risk::sigma_xy(Some(b.net), q, x, y).math()?` (liquidate.rs:345-346). price_wad(x, y) = ⌊y·WAD/x⌋ is 0 when y·10¹⁸ < x (y <= 18 lamports for x <= u64::MAX); risk::sigma then divides n·WAD·10⁴ by q·0, returns DivByZero, and liquidate and expire revert. arming::RiskQuotes::sigma_of maps the same DivByZero to u64::MAX (arming.rs:101-109) and VenueState::price() filters p > 0, so only the keeper exits fail.

#### `shortwin|liquidate|hardening-unmapped-sigma-div-by-zero` — LEAD — `settlement:1-math-precision`

- Location: `programs/shortwin/src/instructions/liquidate.rs:346`
- Description: `assess` stops `liquidate` and `expire` with `MathOverflow` when the live price floors to 0, where `sol_settle` reads σ as unbounded. No eligible pool reaches that price today.
- Path: programs/shortwin/src/instructions/liquidate.rs
- Fix: Map `MathError::DivByZero` from `risk::sigma_xy` to `u64::MAX` in `assess`, as `RiskQuotes::sigma_of` does.
- Code smells: `risk::sigma_xy(Some(b.net), q, u128::from(state.x), u128::from(state.y)).math()?` is guarded only by `state.x > 0`; `price_wad` = ⌊y·WAD/x⌋ is 0 whenever x > 10¹⁸·y (possible inside u64 for a mint with more than 10¹⁸ raw units and a pool holding under 18 lamports), and `sigma` then returns `DivByZero`, mapped to `MathOverflow`. `arming::RiskQuotes::sigma_of` maps the same `DivByZero` to `u64::MAX`. Today both paths refuse the exit (σ = ∞ would give `UseSolSettle`), so only the error code differs; a later change that reads σ outside the gate would make every keeper exit on such a pool fail.

#### `shortwin|open-short|curve-open-ignores-pamm-buy-halt` — LEAD — `positions:8-asymmetry`

- Location: `programs/shortwin/src/instructions/open_short.rs:332`
- Description: open_short checks only the PumpSwap bit that halts create_pool on a curve open, so a borrower opens a curve short while PumpSwap halts every buy. Such a position has no exit after graduation until S3; we assume the owner meant to gate only create_pool.
- Path: PumpSwap admin sets GlobalConfig.disable_flags = 0x08 (buy halted, create_pool open) -> borrower calls open_short on a PumpCurve market: the curve branch tests only bit 0 and passes -> the curve completes (any buyer, or another position's q >= rT buyback) -> migrate_v2 succeeds -> the prologue's pump_curve::sync moves the market to PumpSwap -> close_short / liquidate / expire hit exit_venue_checks `require!(state.executable, NoRoute)`, which is false while the buy bit is set -> the position has no exit until S3 at FV.
- Proof: open_short.rs:316 (PumpSwap open): `require!(pool.disable_flags == 0, ShortwinError::VenueFlagged)` - every halt bit refuses. open_short.rs:331-334 (curve open): `pricing.pool_disable_flags & venues::pump_amm::PAMM_CREATE_POOL_DISABLED_BIT == 0` - bit 0 only. shortwin-venue-state pump_amm.rs:476/481: DISABLE_BUY_BIT = 1 << 3, DISABLE_CREATE_POOL_BIT = 1 << 0. venues/pump_amm.rs module docs: the PumpSwap venue predicate requires `disable_flags & DISABLE_BUY_BIT == 0`. With flags = 0x08: curve check 0x08 & 0x01 = 0, so the open passes; PumpSwap check 0x08 != 0, so it refuses. M6-SEC-3 gives the create_pool rationale ('a curve that completes under it cannot graduate'). The same rationale applies to the buy bit, because a curve that completes under it graduates onto a pool with no buy route.
- Fix: Refuse a curve open when the buy bit of PumpSwap is set as well as the create_pool bit.
- Code smells: The curve and PumpSwap branches of the same open gate read the same disable_flags byte with different masks. This is related to, but distinct from, the known venue-gate-mask-asymmetry lead, which concerns short.win's own venue_enabled and route-broken bits.

```diff
--- a/programs/shortwin/src/instructions/open_short.rs
+++ b/programs/shortwin/src/instructions/open_short.rs
@@
             require!(
-                pricing.pool_disable_flags & venues::pump_amm::PAMM_CREATE_POOL_DISABLED_BIT == 0,
+                pricing.pool_disable_flags
+                    & (venues::pump_amm::PAMM_CREATE_POOL_DISABLED_BIT
+                        | venues::pump_amm::PAMM_BUY_DISABLED_BIT)
+                    == 0,
                 ShortwinError::GraduationHalted
             );
```

#### `shortwin|open-short|hardening-implied-owner` — LEAD — `positions:9-account-validation`

- Location: `programs/shortwin/src/instructions/open_short.rs:166`
- Description: open_short accepts pool_vault on its PDA seeds alone, with no token::mint or token::authority check, so a change to vault creation removes that guarantee.
- Fix: Add token::mint = mint, token::authority = market and token::token_program = token_program to pool_vault in open_short and the other trading instructions.
- Code smells: pool_vault is an InterfaceAccount<TokenAccount> bound only by `seeds = [POOL_VAULT_SEED, market]` with the stored pool_vault_bump and has_one on market. It has no token::mint = mint, token::authority = market or token::token_program constraint. Its mint and authority are trusted only because create_market made the PDA by hand with InitializeAccount3 (create_market.rs:223).

#### `shortwin|open-short|lagging-ema-open-margin` — LEAD — `positions:5-invariant`

- Location: `programs/shortwin/src/instructions/open_short.rs:343`
- Description: Right after a fast genuine pump, P_ema trails the pool, so a sale wrapped around open_short margins the short below its real buyback cost. We did not prove a profit at real keeper timing or for the size of token holding the push needs.
- Fix: Price the D4 margin also at the last completed bucket's high, so a sale inside the open cannot margin below the observed price.
- Code smells: spot_near_ema compares the pre-trade spot with the post-roll P_ema, and a sale earlier in the same transaction moves that spot. The EMA step is clamped (aggregate() bounds the observation to x(1 + C); with C = 100 % and dt = 1, P_ema rises by at most 7/6 per second), so after a genuine 5x jump P_ema is 1.17x after 1 s and 2.16x after 5 s, even with a keeper crank every second. The D4 EMA leg (newest_ring_xy) reads the same trailing ring entry, and N_open is lowered by the wrapping sale (KI-12). The margin never reads the last completed bucket's high (lb_hi), which honest per-second cranks keep at the observed price.

#### `shortwin|open-short|missing-locked-terms-bound` — LEAD — `positions:11-trust-gap`

- Location: `programs/shortwin/src/instructions/open_short.rs:216 (handler loads config with no version bind); terms locked at :364 and :606`
- Description: Any caller can apply a pending config change just before a borrower's open_short, so the position locks terms that its bounds do not check. We did not show a caller profit beyond the terms Governance approved.
- Seam: access×economics×asymmetry: any caller decides when a timelocked Params change lands (apply_config has no signer); open_short locks the config read in that instruction for the position's whole life (A26); the borrower's bounds cover only S, M and the open fee
- Path: programs/shortwin/src/instructions/open_short.rs
- Proof: Pending Params change: Tier A cr_liq_bps 12,500 -> 18,000, cr_open_bps unchanged at 20,000. validate allows this, because cr_open >= cr_liq + 2,000. Block order: [apply_config (any caller), open_short (victim)]. The victim's M = ceil(2.0*B) - S and the open fee are the same as the client quoted, so max_margin and max_open_fee pass. The position locks cr_liq 18,000: it becomes liquidatable after about an 11 % rise in B (CR 200 % -> 180 %) instead of about 60 % (-> 125 %). The same ordering also changes the locked penalty_bps, rate_*_bps, max_duration_s (bounds 3,600..7,776,000), min_rent_s, rent_protocol_bps, the keeper terms and the S3 timers. None of these is bounded.
- Fix: Add an expected_params_version argument to open_short and refuse the open when config.params_version differs.
- Code smells: open_short(nonce, q, min_proceeds, max_margin, max_open_fee, extras) has no config-version argument (lib.rs:213-231). The handler copies config.tiers[tier] (open_short.rs:364) and config.params (pos.lock_terms, :606) into the position for its whole life. apply_config needs no signer and runs at any time in [not_before, not_before + 14 d] (apply_config.rs:1-2). GlobalConfig.apply bumps params_version on every Params change (state/config.rs:174), and no instruction reads it.

```diff
--- a/programs/shortwin/src/lib.rs
+++ b/programs/shortwin/src/lib.rs
@@
         max_open_fee: u64,
         extras: venues::ExtrasSel,
+        expected_params_version: u32,
     ) -> Result<()> {
         instructions::open_short::handler(
             ctx,
             nonce,
             q,
             min_proceeds,
             max_margin,
             max_open_fee,
             extras,
+            expected_params_version,
         )
     }
--- a/programs/shortwin/src/instructions/open_short.rs
+++ b/programs/shortwin/src/instructions/open_short.rs
@@
     max_open_fee: u64,
     extras: ExtrasSel,
+    expected_params_version: u32,
 ) -> Result<()> {
     require!(q > 0, ShortwinError::ZeroAmount);
     let clock = Clock::get()?;
     let market_key = ctx.accounts.market.key();
     let position_key = ctx.accounts.position.key();
     let borrower_key = ctx.accounts.borrower.key();
     let config = ctx.accounts.config.load()?;
+    require_eq!(
+        config.params_version,
+        expected_params_version,
+        ShortwinError::ConfigVersion
+    );
```

#### `shortwin|sol-settle|hardening-unchecked-writable-account` — LEAD — `settlement:9-account-validation`

- Location: `programs/shortwin/src/instructions/sol_settle.rs:151`
- Description: sol_settle checks the share_epoch_rec key only when the epoch rolls, so on other calls the caller passes any writable account there. A future use before the check would act on that account.
- Fix: Check share_epoch_rec against the PDA of the current share epoch at the start of every call.
- Code smells: `share_epoch_rec: UncheckedAccount` is #[account(mut)] with no constraint. Its key is compared with PDA([share_epoch, market, epoch]) only inside `if let Some(mut rec) = b.rec` (sol_settle.rs:416). Anchor 1.2's duplicate-mutable check covers only Account, InterfaceAccount, LazyAccount and Migration fields. So on a call that does not roll, the caller can pass any writable account there, including exposure, market_vault or the borrower. Nothing reads or writes the account then, so no path exists today.

```diff
--- a/programs/shortwin/src/instructions/sol_settle.rs
+++ b/programs/shortwin/src/instructions/sol_settle.rs
@@ -353,6 +353,15 @@ pub fn handler<'info>(ctx: Context<'info, SolSettle<'info>>) -> Result<()> {
     let clock = Clock::get()?;
     let market_key = ctx.accounts.market.key();
+    // The record a roll of the current epoch would create: checked on every call.
+    let epoch_le = ctx.accounts.market.load()?.share_epoch.to_le_bytes();
+    let (rec_key, _) =
+        Pubkey::find_program_address(&[ShareEpoch::SEED, market_key.as_ref(), &epoch_le], &crate::ID);
+    require_keys_eq!(
+        ctx.accounts.share_epoch_rec.key(),
+        rec_key,
+        ShortwinError::EpochRecordMismatch
+    );
```

#### `shortwin|withdraw|dust-position-blocks-last-lender-flush` — LEAD — `lending:3-economic-security`

- Location: `programs/shortwin/src/instructions/withdraw.rs:207`
- Description: One open position of one raw unit stops the last lender's full exit, so the lender cannot take the unreleased vest at once. We did not check the smallest position that the open caps allow, or how fast the keeper expires it.
- Path: programs/shortwin/src/instructions/withdraw.rs
- Fix: Make the S = 0 credit path safe while tokens are borrowed, so that the last lender's full exit and the vest flush do not depend on every position being closed.
- Code smells: withdraw.rs:207 requires `pool.total_shares != 0 || pool.borrowed_q == 0` (LastSharesBorrowed). The last-lender vest flush in withdraw_in (`shares == p.total_shares && p.vest_bal != 0`) can only complete when the sum of borrowed tokens is 0. One open position of any size keeps that sum above 0 until the position exits, up to its deadline, so the last lender keeps at least one share and gets the vest only through the linear release.

#### `shortwin|withdraw|hardening-implied-owner` — LEAD — `lending:9-account-validation`

- Location: `programs/shortwin/src/instructions/withdraw.rs:132`
- Description: withdraw and deposit check pool_vault only by its PDA, not by token::mint or token::authority, so a later change of its creation removes both checks.
- Path: programs/shortwin/src/instructions/withdraw.rs
- Fix: Add token::mint = mint and token::authority = market to pool_vault in Deposit and Withdraw.
- Code smells: withdraw.rs:127-132 and deposit.rs:94-99 constrain pool_vault only by seeds = [POOL_VAULT_SEED, market] with bump = market.pool_vault_bump, plus has_one on market. The mint and the market authority come only from create_market's InitializeAccount3 (create_market.rs:275). The program has no SetAuthority or CloseAccount on it today, so there is no current path. The listed Liquidate lead is the same class.

```diff
--- a/programs/shortwin/src/instructions/withdraw.rs
+++ b/programs/shortwin/src/instructions/withdraw.rs
         seeds = [POOL_VAULT_SEED, market.key().as_ref()],
-        bump = market.load()?.pool_vault_bump
+        bump = market.load()?.pool_vault_bump,
+        token::mint = mint,
+        token::authority = market,
+        token::token_program = token_program
     )]
     pub pool_vault: Box<InterfaceAccount<'info, TokenAccount>>,
```

#### `shortwin|withdraw|missing-lock-bound` — LEAD — `lending:8-asymmetry`

- Location: `programs/shortwin/src/instructions/withdraw.rs:204 (only min_tokens / min_sol); crates/shortwin-math/src/pool.rs:1813 (add sized on current T, ΣB_Q); pool.rs:1931 (decay only when bn < b_min)`
- Description: withdraw bounds the tokens and SOL it pays but not the lock it adds, so a front-run open can lock the lender's shares for months. We did not prove a net profit for the attacker.
- Path: programs/shortwin/src/instructions/withdraw.rs
- Proof: The lock add is sized on the current state: add = ceil(min(shares, b_max)*(u - b_max)/b_max), with b_max taken from the allowance on the current T and ΣB_Q. It decays only when resolve sees bn < b_min, the running minimum since the lock era started (V2-3), and its lock_until uses max_release. Numbers: S = 1e9, T = 9e8, ΣB_Q = 1e8 = b_min, and lender W has a live lock of 5e7, so lock_norm > 0 and no era reset happens. V holds 5e8 unlocked shares and withdraws 1e8 shares with min_tokens = 1e8. Without a front-run, V's allowance is 473,684,210 tokens and add ≈ 5.56e6 shares. With a front-run open of q = 7e8 at maximum duration (T = 2e8, ΣB_Q = 8e8): V's allowance is 105,263,157 >= 1e8, so V's transaction still passes. add ≈ ceil(1e8*3.947e8/1.0526e8) ≈ 3.75e8 shares, so 3.75e8 of V's remaining 4e8 shares lock, with lock_until ≈ the attacker's release_at + lock_grace_s (≤ 94 d). The attacker then closes. resolve(q = 7e8) gives bn = 1e8, which is not < b_min, so Π does not decay and V stays locked. Meanwhile locked rises by ≈ 3.75e8, su falls, and W's allowance takes most of the free T.
- Fix: Add a max_lock argument to withdraw and refuse the withdrawal when the new lock or lock_until exceeds it.
- Code smells: pair_or_branch: deposit (min_shares) and withdraw (min_tokens, min_sol) vs the lock that withdraw writes. asymmetry: the lock is sized at the current ΣB_Q but decays from the era minimum b_min, and the caller cannot bound it.

```diff
--- a/programs/shortwin/src/instructions/withdraw.rs
+++ b/programs/shortwin/src/instructions/withdraw.rs
@@ pub fn handler<'info>(
     min_tokens: u64,
     min_sol: u64,
+    max_lock: u128,
 ) -> Result<()> {
@@
         require_gte!(tokens, min_tokens, ShortwinError::TokensBelowMin);
+        require!(l.lock <= max_lock, ShortwinError::LockAboveMax);
```

## 2. FINDINGs on keys that pass 1 judged as leads

Pass 1's lead: The allowance value cap counts a lender's locked shares

#### `shortwin-math|allowance|allowance-cap-counts-locked-shares` — FINDING — `lending:1-math-precision`

- Location: `crates/shortwin-math/src/pool.rs:1756`
- Description: A locked lender deposits one token and takes free tokens worth its whole position, because allowance counts its locked shares in the value cap.
- Path: Setup reaches the over-locked state that AIR-11 allows; every open passes caps::util_ok at the default 9,000 bps. E deposits 1,000,000. P0 borrows 100,000. E withdraws 400,000, the first lock of the era, so b_min = 100,000. L deposits 19,000,000. P1 borrows 17,500,000. E and L each withdraw their whole allowance, so both are fully locked. P1 repays: ΣB_Q = 100,000 = b_min, so resolve_in does not decay Π. Now T = 17,500,000 and both allowances are 0. Attack: L sends [deposit(amount=1, min_shares=0), withdraw(shares=17,056,818,099 = all its shares, min_tokens=0, min_sol=0)] in one transaction. pool::withdraw accepts because amount <= allowance = the value of all L's shares. L burns its 17,056,817,100 locked shares and receives 17,056,819 tokens. E keeps the 100,000 borrowed tokens alone and later takes all of P0's loss.
- Proof: Code: pool.rs:1749-1757 computes u = shares - lock_of(...), su = max(S - locked_total, u), by_free = floor(u*T/su), and by_value = tokens_for_shares(pool, lender.shares). by_value counts all shares, locked ones included. withdraw_in (pool.rs:1802-1805) checks only amount <= allow, never shares <= u. PoC (scratch copy of the crate, public pool API): after P1 repays, T = 17,500,000, S = 17,599,998,101, locked_total = S. E holds 543,181,001 shares and L holds 17,056,817,100, all locked. Both allowances are 0. L's 1-token deposit mints 999 shares. Then u = 999, su = max(17,599,999,100 - 17,599,998,101, 999) = 999, by_free = 17,500,001, by_value = 17,056,819, so allowance = 17,056,819. L withdraws all 17,056,818,099 shares and receives 17,056,819 tokens. Next P0 is settled S1 at 50 % (50,000 lamports for 100,000 tokens). E ends with 493,181 (tokens + lamports). With no deposit trick the same events give E 541,637 and L 17,008,361. So E loses 48,456 (8.9 % of its value) and L gains 48,457 net of its 1 token. E's share of P0's exposure goes from 3.1 % to 100 %; KI-13 measures about 33 % for the disclosed run residual. L leaves a lock that ran to day 322, about 20 days early. With by_value = tokens_for_shares(pool, u) the same run gives L 0 tokens for its 999 shares and E 541,637. The crate's pool unit tests still pass with that change; 8 pool_vectors cases (pool_model.py vectors) change.
- Fix: Cap the allowance at the value of the unlocked shares u, and change pool_model.py free_allowance and the pool vectors to match.
- Code smells: The value cap uses lender.shares, not u. withdraw_in checks tokens, not shares, against the lock. AIR-11 over-locking leaves su small while T is large. deposit and withdraw run in one transaction with no cooldown.

```diff
--- a/crates/shortwin-math/src/pool.rs
+++ b/crates/shortwin-math/src/pool.rs
@@ -1754,5 +1754,7 @@ pub fn allowance(pool: &Pool, vault: &Vault, lender: &Lender, now: i64) -> Result<u64, MathError> {
     let t = u128::from(pool.tokens_free);
     let by_free = mul_div(u, t, su, Rounding::Floor)?;
-    let by_value = tokens_for_shares(pool, lender.shares)?;
+    // A lock keeps the locked shares' value in the pool: cap the free part at the value of
+    // the unlocked shares (with all shares, a 1-token deposit burns a lock; pool_model too).
+    let by_value = tokens_for_shares(pool, u)?;
     to_u64(by_free.min(by_value).min(t))
 }
```

Pass 1's lead: The allowance value cap counts a lender's locked shares

#### `shortwin-math|allowance|allowance-cap-counts-locked-shares` — FINDING — `lending:7-first-principles`

- Location: `crates/shortwin-math/src/pool.rs:1756`
- Description: A locked lender deposits one token, then withdraws its whole position as free tokens, because the allowance counts its locked shares.
- Path: crates/shortwin-math/src/pool.rs
- Proof: assumption: a lock keeps a lender's borrowed-part shares from being exchanged for free tokens until Π decays or lock_until passes (C0-9, R3B-1; ECON §9.3 pro-rata allowance). violation: `by_value = tokens_for_shares(pool, lender.shares)` (pool.rs:1756) counts locked shares, `su = max(S − locked, u)` (pool.rs:1750) collapses to `u` when every co-lender is locked, and withdraw_in only checks `shares <= l.shares` (pool.rs:1793) and `amount <= allow`, so it burns locked shares; `new_lock = min(cur + add, remaining)` (pool.rs:1824) then shrinks the lock. Run of the unmodified crates/shortwin-math (copy at e035f2e; <scratch>): A, B, C deposit 1,000,000 tokens each; a 10,000-token position opens; C withdraws 1,000 tokens (first lock of the era, b_min = 10,000, Π = 1e18); two 1,000,000-token positions open; A, B, C each withdraw their allowance (329,776 / 329,777 / 329,447) and are fully locked (lock_of = shares). One large position repays: T = 1,000,000, ΣB_Q = 1,010,000 ≥ b_min, so Π stays 1e18 and every allowance is 0 (a fair third of T = 333,333). A deposits 1 token (999 shares, pool not drained): allowance(A) = 670,224 = the value of all 670,224,000 of its shares (u = 999, su = 999, by_free = T). A withdraws 670,224 tokens, burning 670,224,000 shares, including 670,223,001 locked shares, and holds 0 shares. B and C keep allowance 0 with T = 329,777 and all 1,010,000 borrowed tokens. When the last large position settles for 0 SOL, B's value falls 670,222 -> 169,973 and C's 669,553 -> 169,803. With the fix below, A's allowance after the 1-token deposit is 0, and all three end at 336,779. So A moved its 333,445-token share of the loss (49.8 % of its position) onto B and C, above KI-13's disclosed +11.1 % run residual. KI-13 discloses only that genuinely unlocked co-lenders exit first, and in this run every lender was over-locked. In the program both steps are owner-signed `deposit(1)` then `withdraw(all shares)` (no cooldown); the over-locked state comes from the inherited b_min that KI-13/AIR-11 describe.
- Fix: Cap the value term of the allowance at the value of the unlocked shares `u`, so a withdrawal never spends locked shares.

```diff
--- a/crates/shortwin-math/src/pool.rs
+++ b/crates/shortwin-math/src/pool.rs
@@ -1754,5 +1754,7 @@ pub fn allowance(pool: &Pool, vault: &Vault, lender: &Lender, now: i64) -> Result<u64, MathError> {
     let t = u128::from(pool.tokens_free);
     let by_free = mul_div(u, t, su, Rounding::Floor)?;
-    let by_value = tokens_for_shares(pool, lender.shares)?;
+    // C0-9: cap at the unlocked shares' value, so a withdrawal never spends locked shares
+    // (in a consistent state ⌊u·T/su⌋ is already below it; it binds only when su collapses).
+    let by_value = tokens_for_shares(pool, u)?;
     to_u64(by_free.min(by_value).min(t))
 }
```

Pass 1's lead: A one-unit position stops the last lender's vest flush

#### `shortwin-math|withdraw|dust-position-blocks-last-lender-flush` — FINDING — `lending:4-execution-trace`

- Location: `crates/shortwin-math/src/pool.rs:1855`
- Description: A borrower keeps a small position open so the last lender cannot exit in full, then closes it and deposits to take that lender's vest.
- Path: crates/shortwin-math/src/pool.rs
- Proof: Code: pool.rs:1855 `if shares == p.total_shares && p.vest_bal != 0 {` pays the R3B-2 flush only when the burn takes every share. While ΣB_Q > 0 the last lender cannot burn every share: `tokens_for_shares(S)` exceeds T (InsufficientInventory), and withdraw.rs:206-208 refuses S = 0 with `LastSharesBorrowed`. PoolDrained (pool.rs:1971) depends on ΣB_Q > 0, which the borrower ends by closing its position. Trace on shortwin-math e035f2e: A deposits 1,000,000 raw. Shorts of 800,000 and 10,000 open; the 10,000 short is the attacker's and stays above min_notional. The 800,000 short is liquidated with a 10 SOL lump that vests over 6 h. A, the only lender, withdraws its whole allowance (990,000 tokens) and keeps 9,999,001 shares, all locked; vest_bal is 10 SOL. The attacker sends [close_short(10,000), deposit(100,000)] in one transaction. The deposit is accepted (ΣB_Q = 0) and mints 99,990,010 shares. After 6 h the attacker is paid 9,090,909,090 lamports and withdraws 99,999 tokens; A is paid 909,090,909. With a 1-unit position, a 1,000-raw deposit takes 9,999,980,020 of the 10 SOL (A gets 19,979); PoolDrained refuses that deposit only while the position is open. ARCH §3.3 states: 'Last lender (R3B-2): if shares == S and vest_bal > 0, release the whole vest ... so the leaving lender is paid it'. Known LEAD from an earlier scan, now with a trace.
- Fix: Pay the vest to the lender whenever it holds every share (`l.shares == S`), not only when it burns every share.

```diff
--- a/crates/shortwin-math/src/pool.rs
+++ b/crates/shortwin-math/src/pool.rs
@@ fn withdraw_in(
-    if shares == p.total_shares && p.vest_bal != 0 {
+    // R3B-2: the lender that holds every share is paid the unreleased vest at any withdrawal,
+    // so an open position that keeps its last shares in the pool cannot leave the vest to a
+    // later deposit (KI-65: l.shares == S only when no other position holds or will hold one).
+    if l.shares == p.total_shares && p.vest_bal != 0 {
```

## 3. Blocks that restate a pass-1 key

| Key | Pass 1 | Agent | Kind | Description |
|---|---|---|---|---|
| `shortwin-math|allowance|allowance-cap-counts-locked-shares` | LEAD | `lending:10-numerical-gap` | LEAD | allowance caps by the value of all the lender's shares, locked ones included, so a 90 %-locked lender withdraws every share. This needs more free tokens per unlocked share than the share price, which over-locking (KI-13) can create. |
| `shortwin-math|allowance|allowance-cap-counts-locked-shares` | LEAD | `lending:11-trust-gap` | LEAD | A lender whose unlocked shares are the only unlocked shares can withdraw its locked shares too, because the allowance value cap counts every share. The locked lenders lose free tokens now, and lose value only if their borrowed tokens later settle at a loss. |
| `shortwin-math|allowance|allowance-cap-counts-locked-shares` | LEAD | `lending:12-flow-gap` | LEAD | A lender with one unlocked share burns locked shares for all free tokens, because the value cap counts its locked shares. We did not measure this against KI-13's bound. |
| `shortwin-math|allowance|allowance-cap-counts-locked-shares` | LEAD | `lending:3-economic-security` | LEAD | allowance caps the withdrawal at the value of all the lender's shares, so a locked lender can take free tokens beyond its unlocked part. We did not prove a reachable state where the co-lenders stay locked while the free tokens are large. |
| `shortwin-math|allowance|allowance-cap-counts-locked-shares` | LEAD | `lending:4-execution-trace` | LEAD | allowance caps a withdrawal at the value of all the lender's shares, locked ones included, so one unlocked share can withdraw tokens its lock holds. We did not build a state where this moves value between lenders. |
| `shortwin-math|allowance|allowance-cap-counts-locked-shares` | LEAD | `lending:6-periphery` | LEAD | The allowance caps a withdrawal at the value of all the lender's shares, so a lender with one unlocked share takes its locked value too. We did not prove a loss beyond KI-13's over-lock. |
| `shortwin-math|allowance|allowance-cap-counts-locked-shares` | LEAD | `lending:9-account-validation` | LEAD | allowance caps a withdrawal at the value of all the lender's shares, locked ones too, so a lender can burn locked shares for free tokens. We did not rebuild a case where it takes another lender's tokens. |
| `shortwin-math|convert-steps|lock-rounds-down-at-conversion` | FINDING 90 | `lending:1-math-precision` | FINDING | The rebase conversion floors a lender's lock but sometimes rounds its shares up, so a fully locked lender withdraws free tokens worth its whole position. |
| `shortwin-math|convert-steps|lock-rounds-down-at-conversion` | FINDING 90 | `lending:10-numerical-gap` | LEAD | A rebase floors each lock alone but floors the bucket as a sum, so total allowances can pass T by T*m/su. We found this material only in a pool that is almost fully locked. |
| `shortwin-math|convert-steps|lock-rounds-down-at-conversion` | FINDING 90 | `lending:11-trust-gap` | LEAD | A rebase floors a locked lender's lock but may round its shares up, so a fully locked lender leaves the conversion with an unlocked share. We did not build the state where that share reaches all free tokens. |
| `shortwin-math|convert-steps|lock-rounds-down-at-conversion` | FINDING 90 | `lending:12-flow-gap` | FINDING | A rebase floors a full lock but rounds the shares up, so the second lender to convert can withdraw all free tokens. |
| `shortwin-math|convert-steps|lock-rounds-down-at-conversion` | FINDING 90 | `lending:3-economic-security` | FINDING | At a rebase, convert_steps floors the lender's own lock, so a locked lender gets an unlocked share and withdraws free tokens that its lock reserved. |
| `shortwin-math|convert-steps|lock-rounds-down-at-conversion` | FINDING 90 | `lending:4-execution-trace` | FINDING | A rebase floors a lender's lock but can round its shares up, so a fully locked lender gets an unlocked share and withdraws free tokens. |
| `shortwin-math|convert-steps|lock-rounds-down-at-conversion` | FINDING 90 | `lending:6-periphery` | FINDING | The second lender to convert through a rebase record keeps one unlocked share, because the lock floors, and then withdraws every free token. |
| `shortwin-math|convert-steps|lock-rounds-down-at-conversion` | FINDING 90 | `lending:7-first-principles` | FINDING | A rebase conversion floors a locked lender's lock but can round its shares up, so the lender withdraws free tokens that others should share. |
| `shortwin-math|convert-steps|lock-rounds-down-at-conversion` | FINDING 90 | `lending:9-account-validation` | LEAD | A rebase floors the lender's lock but can round its shares up, so a fully locked lender leaves the conversion with an unlocked share. We did not rebuild the earlier scan's exploit with numbers. |
| `shortwin-math|evaluate|arm-clock-survives-restart` | FINDING 75 | `settlement:3-economic-security` | FINDING | The warm-up keeps a clock armed on the old pool, so a keeper executes S1 or S2 after about three updates on the new pool. |
| `shortwin-math|evaluate|arm-clock-survives-restart` | FINDING 75 | `settlement:4-execution-trace` | FINDING | A keeper settles a position in SOL one evaluation after a pool switch, because the arming count still holds the old pool's updates. |
| `shortwin-math|evaluate|arm-clock-survives-restart` | FINDING 75 | `settlement:5-invariant` | FINDING | A keeper executes S1 or S2 right after a venue restart, because the warm-up gate leaves an old arming clock live with its old count. |
| `shortwin-math|evaluate|arm-clock-survives-restart` | FINDING 75 | `settlement:6-periphery` | FINDING | Any keeper can execute S1 or S2 at the end of a restart's warm-up, because evaluate keeps a clock that the old pool's evidence armed. |
| `shortwin-math|evaluate|arm-clock-survives-restart` | FINDING 75 | `settlement:7-first-principles` | FINDING | A clock armed before a venue restart stays live through the warm-up, so its old count lets S1 or S2 execute without the persistence. |
| `shortwin-math|evaluate|arm-clock-survives-restart` | FINDING 75 | `settlement:8-asymmetry` | FINDING | evaluate keeps an arm clock from before a venue restart, so S1 or S2 executes on the first evaluation after the warm-up. |
| `shortwin-math|evaluate|arm-clock-survives-restart` | FINDING 75 | `settlement:9-account-validation` | FINDING | A clock armed before a venue restart stays live through the warm-up, so a keeper executes S1 after one evaluation of the new pool. |
| `shortwin-math|expire|expiry-reward-below-liquidation-floor` | LEAD | `lending:7-first-principles` | LEAD | A keeper that waits until an expired position becomes liquidatable earns K_s instead of the smaller K_e that expire pays. We did not prove a lender loss while the in-house keeper runs. |
| `shortwin-math|expire|expiry-reward-below-liquidation-floor` | LEAD | `settlement:1-math-precision` | LEAD | The expiry waterfall pays the keeper K_e, which is 2,000,000 lamports where a later liquidation pays 30,000,000, so a keeper can wait. We did not prove that the wait profits. |
| `shortwin-math|expire|expiry-reward-below-liquidation-floor` | LEAD | `settlement:10-numerical-gap` | LEAD | A keeper earns about 14 times more from a later liquidation than from expire. We did not prove that keepers delay expiries for it. |
| `shortwin-math|expire|expiry-reward-below-liquidation-floor` | LEAD | `settlement:4-execution-trace` | LEAD | A keeper earns about ten times more from a later liquidation than from expire, so it can wait and make the borrower pay the penalty. |
| `shortwin-math|expire|expiry-reward-below-liquidation-floor` | LEAD | `settlement:5-invariant` | LEAD | A keeper earns about 0.002 SOL from expire, and at least K_s from a later liquidation that leaves equity, so keepers can wait. |
| `shortwin-math|expire|expiry-reward-below-liquidation-floor` | LEAD | `settlement:6-periphery` | LEAD | expire pays the keeper about one tenth of the liquidation floor, so a keeper that waits for rent to force a liquidation earns more. We did not prove that one keeper can wait while other keepers compete. |
| `shortwin-math|expire|expiry-reward-below-liquidation-floor` | LEAD | `settlement:7-first-principles` | LEAD | expire pays the keeper K_e, far below the liquidation floor K_s, so a keeper gains more if it waits until the position is CR-liquidatable. We did not prove that rent or price moves make the wait profitable in practice. |
| `shortwin-math|expire|expiry-reward-below-liquidation-floor` | LEAD | `settlement:8-asymmetry` | LEAD | A keeper earns more from a later liquidation than from expire, so it can wait and make the borrower pay the penalty. We did not prove that keepers wait in practice. |
| `shortwin-math|expire|expiry-reward-below-liquidation-floor` | LEAD | `settlement:9-account-validation` | LEAD | A keeper earns about twelve times more from a liquidation than from an expiry, so keepers wait for an expired position to become CR-liquidatable. We did not prove that a keeper can make that happen. |
| `shortwin-math|liquidate|keeper-floor-junior-to-rent` | FINDING 55 | `lending:7-first-principles` | FINDING | The liquidation pays rent before the keeper floor, so a keeper earns nothing when rent takes V − B and waits for S1. |
| `shortwin-math|liquidate|keeper-floor-junior-to-rent` | FINDING 55 | `settlement:1-math-precision` | FINDING | When rent takes all of V − B, the liquidation waterfall pays the keeper 0 lamports, so keepers wait for S1 and lenders lose more. |
| `shortwin-math|liquidate|keeper-floor-junior-to-rent` | FINDING 55 | `settlement:10-numerical-gap` | FINDING | A keeper earns less from liquidate than from S1 when the buyback nears V − K_s, because rent and the penalty shares come before the keeper floor. |
| `shortwin-math|liquidate|keeper-floor-junior-to-rent` | FINDING 55 | `settlement:3-economic-security` | FINDING | When rent takes all of `V − B`, liquidate pays the keeper nothing, so keepers skip the position and lenders wait for an S1. |
| `shortwin-math|liquidate|keeper-floor-junior-to-rent` | FINDING 55 | `settlement:4-execution-trace` | FINDING | A keeper receives nothing for a valid liquidation when rent due exceeds the vault left after the buyback, so third-party keepers skip it. |
| `shortwin-math|liquidate|keeper-floor-junior-to-rent` | FINDING 55 | `settlement:5-invariant` | FINDING | A keeper who liquidates a position whose rent takes the whole remainder after the buyback receives nothing, so no keeper has a reason to act. |
| `shortwin-math|liquidate|keeper-floor-junior-to-rent` | FINDING 55 | `settlement:6-periphery` | FINDING | A keeper who liquidates a position whose rent takes the whole remainder gets 0 lamports, because the waterfall pays rent before the keeper floor. |
| `shortwin-math|liquidate|keeper-floor-junior-to-rent` | FINDING 55 | `settlement:7-first-principles` | FINDING | When rent takes all of V − B, liquidate pays the keeper nothing, so no keeper liquidates the position until it reaches S1. |
| `shortwin-math|liquidate|keeper-floor-junior-to-rent` | FINDING 55 | `settlement:8-asymmetry` | FINDING | waterfall::liquidate pays rent before the keeper floor, so a keeper gets nothing when the rent takes the whole remainder. |
| `shortwin-math|liquidate|keeper-floor-junior-to-rent` | FINDING 55 | `settlement:9-account-validation` | FINDING | A keeper who liquidates a position whose rent due takes the remainder after the buyback receives no lamports, so keepers leave the position open. |
| `shortwin-math|settle|flush-skips-post-wipe-sweep-gate` | FINDING 80 | `lending:1-math-precision` | FINDING | After a settlement flushes the vest on dust claims, a new depositor takes most of the old lenders' trader-side SOL that sweep_trader books later. |
| `shortwin-math|settle|flush-skips-post-wipe-sweep-gate` | FINDING 80 | `lending:12-flow-gap` | FINDING | After a dust flush leaves shares, deposit skips the sweep gate, so a new depositor takes part of the old lenders' unswept trader SOL. |
| `shortwin-math|settle|flush-skips-post-wipe-sweep-gate` | FINDING 80 | `lending:6-periphery` | FINDING | After a dust settlement flushes the vest, a new depositor enters at once and takes the trader SOL that the old lenders earned. |
| `shortwin-math|settle|flush-skips-post-wipe-sweep-gate` | FINDING 80 | `lending:8-asymmetry` | FINDING | A depositor after a dust-flush settlement takes most of the pre-flush venue cashback, because only a full wipe makes deposits wait for the sweeps. |
| `shortwin-math|settle|flush-skips-post-wipe-sweep-gate` | FINDING 80 | `lending:9-account-validation` | FINDING | After a dust flush, a depositor enters before sweep_trader books the trader's cashback, so the new shares take the old lenders' cashback. |
| `shortwin-math|settle|flush-skips-post-wipe-sweep-gate` | FINDING 80 | `settlement:10-numerical-gap` | FINDING | After a dust-flush settlement, a dust depositor takes almost all trader SOL swept later, because pool::settle never clears the post-wipe sweep bits. |
| `shortwin-math|settle|flush-skips-post-wipe-sweep-gate` | FINDING 80 | `settlement:3-economic-security` | FINDING | A settlement that leaves dust flushes the vest but keeps deposits open, so a new depositor takes the cashback that the old positions earned. |
| `shortwin-math|settle|flush-skips-post-wipe-sweep-gate` | FINDING 80 | `settlement:4-execution-trace` | LEAD | After a settlement leaves dust claims, a new depositor gets almost every share and so takes the pre-flush cashback that sweep_trader books later. |
| `shortwin-math|settle|flush-skips-post-wipe-sweep-gate` | FINDING 80 | `settlement:5-invariant` | FINDING | After a settlement leaves dust claims, a new depositor skips the sweep gate and takes trader SOL that the old lenders earned. |
| `shortwin-math|settle|flush-skips-post-wipe-sweep-gate` | FINDING 80 | `settlement:8-asymmetry` | FINDING | A dust-leaving settlement flushes the vest but keeps the sweep bits, so a new depositor takes the old lenders' unswept trader SOL. |
| `shortwin-math|withdraw|dust-position-blocks-last-lender-flush` | LEAD | `lending:10-numerical-gap` | LEAD | An attacker deposits one raw token before the last lender exits, so no flush runs and the attacker later takes the unreleased vest. min_sol turns the exit into a revert, and the lender must wait for vest_end. |
| `shortwin-math|withdraw|dust-position-blocks-last-lender-flush` | LEAD | `lending:12-flow-gap` | LEAD | Any caller keeps a dust position, so the last real lender is never last and its exit leaves the unreleased vest to the dust holder. ECON §9.4 discloses this forfeit (R4B-4). |
| `shortwin-math|withdraw|dust-position-blocks-last-lender-flush` | LEAD | `lending:6-periphery` | LEAD | A one-token holder stops the last-lender vest payout, so the honest full exit forfeits the vest and the holder claims it later. KI-09 discloses forfeiture; the code comment covers only a front-run. |
| `shortwin-math|withdraw|dust-position-blocks-last-lender-flush` | LEAD | `lending:7-first-principles` | LEAD | An attacker keeps a one-token lender position, so the last real lender is never last and its exit leaves the unreleased vest to that position. KI-09 may accept this, and we did not check that the web warns the last lender. |
| `shortwin-math|withdraw|dust-position-blocks-last-lender-flush` | LEAD | `lending:9-account-validation` | LEAD | A one-unit position keeps ΣB_Q above 0, so the last lender cannot burn its last share and withdraw cannot pay it the unreleased vest. |
| `shortwin|claim-sol|hardening-clock-not-clamped` | LEAD | `lending:10-numerical-gap` | LEAD | claim_sol runs the lender touch at the raw clock, not market_now. A clock step back of more than lock_grace_s then makes a withdraw that adds a lock fail. |
| `shortwin|claim-sol|hardening-clock-not-clamped` | LEAD | `lending:11-trust-gap` | LEAD | claim_sol moves the lock calendar at the raw clock, while deposit and withdraw use market_now. We found no wrong value today; a later read of the vest or the allowance there runs on the other time base. |
| `shortwin|claim-sol|hardening-clock-not-clamped` | LEAD | `lending:12-flow-gap` | LEAD | claim_sol passes the raw clock to pool::touch and pool::claim, while deposit and withdraw use market_now. A future check against vest_last then fails after a clock step back. |
| `shortwin|claim-sol|hardening-clock-not-clamped` | LEAD | `lending:2-access-control` | LEAD | claim_sol runs the lender touch and claim at the raw clock, but deposit and withdraw run them at market_now. We found no path today where the two times differ. |
| `shortwin|claim-sol|hardening-clock-not-clamped` | LEAD | `lending:3-economic-security` | LEAD | claim_sol runs the lender touch at the raw clock, while deposit and withdraw use market_now, so the two paths can disagree on lock expiry. |
| `shortwin|claim-sol|hardening-clock-not-clamped` | LEAD | `lending:4-execution-trace` | LEAD | claim_sol runs the lender touch at the raw clock, not market_now, so its lock-expiry test and calendar use a time other than the market's. No path moves value today. |
| `shortwin|claim-sol|hardening-clock-not-clamped` | LEAD | `lending:5-invariant` | LEAD | claim_sol runs the lender touch and claim at the raw clock, not at market_now. A later rule that reads the lock fields sees a time before the last accrual. |
| `shortwin|claim-sol|hardening-clock-not-clamped` | LEAD | `lending:6-periphery` | LEAD | claim_sol touches the lender at the raw clock, not `market_now`, so after a clock step back it reads locks earlier than withdraw does. No caller gains value today. |
| `shortwin|claim-sol|hardening-clock-not-clamped` | LEAD | `lending:7-first-principles` | LEAD | claim_sol runs the lender touch at the raw clock, not at market_now. A future time check against vest_last or last_accrual_ts in this path fails after a clock step back. |
| `shortwin|claim-sol|hardening-clock-not-clamped` | LEAD | `lending:8-asymmetry` | LEAD | claim_sol runs the lender touch at the raw clock, not at market_now, so its calendar advance and lock expiry use a different time base. |
| `shortwin|claim-sol|hardening-clock-not-clamped` | LEAD | `lending:9-account-validation` | LEAD | claim_sol runs the lender touch at the raw clock, not at market_now, so its lock calendar time can differ from deposit and withdraw. |
| `shortwin|claim-sol|hardening-inconsistent-time-base` | LEAD | `lending:1-math-precision` | LEAD | claim_sol moves the lock calendar at the raw clock, not at market_now, so after a clock step back a lock holder's withdraw fails. Agave's clock does not step back today. |
| `shortwin|claim-sol|hardening-inconsistent-time-base` | LEAD | `lending:4-execution-trace` | LEAD | claim_sol advances the lock calendar to the raw clock, which can be past the market's last accrual time that withdraw reads. The difference lasts until the clock passes the dropped day again. |
| `shortwin|claim-sol|hardening-inconsistent-time-base` | LEAD | `lending:7-first-principles` | LEAD | claim_sol moves the lock calendar past the market's last accrual time, so after a clock step back a lock holder's withdraw fails. Agave's clock update prevents the step back today. |
| `shortwin|claim-sol|hardening-inconsistent-time-base` | LEAD | `lending:9-account-validation` | LEAD | claim_sol advances the lock calendar to the current day while Market stays at its last accrual, so the vault and the pool use two times. |
| `shortwin|close-short|hardening-event-destination-hardcoded` | LEAD | `positions:1-math-precision` | LEAD | close_short reports every trader sweep as SWEEP_TO_VEST, so the event names the wrong destination if a sweep ever lands while S is 0. |
| `shortwin|close-short|hardening-event-destination-hardcoded` | LEAD | `positions:3-economic-security` | LEAD | close_short reports every trader sweep as a lender vest, so an indexer reads the wrong destination if the pool has no shares. |
| `shortwin|close-short|hardening-event-destination-hardcoded` | LEAD | `positions:4-execution-trace` | LEAD | The close_short instruction reports swept trader SOL as vested to lenders, but with no shares the pool books that SOL to the protocol. |
| `shortwin|close-short|hardening-event-destination-hardcoded` | LEAD | `positions:6-periphery` | LEAD | close_short reports every trader sweep as SWEEP_TO_VEST, so the event names the wrong destination if the pool has no shares. LastSharesBorrowed keeps S above 0 while a position is open today. |
| `shortwin|close-short|hardening-event-destination-hardcoded` | LEAD | `positions:7-first-principles` | LEAD | close_short always emits TraderSwept with to SWEEP_TO_VEST and wipe_bits 0. The value is correct only while withdraw keeps S above 0 with a position open. |
| `shortwin|close-short|hardening-event-destination-hardcoded` | LEAD | `positions:9-account-validation` | LEAD | close_short reports every trader sweep as SWEEP_TO_VEST, but with no shares pool::sweep books the SOL to protocol_fees, so the event is wrong. |
| `shortwin|close-short|hardening-status-unchecked` | LEAD | `positions:1-math-precision` | LEAD | close_short accepts a position in any status, so a later status such as ExitInProgress does not stop a borrower close. |
| `shortwin|close-short|hardening-status-unchecked` | LEAD | `positions:10-numerical-gap` | LEAD | close_short reads no position status, unlike add_margin, so a borrower can close a position that a v1.1 exit marks ExitInProgress. |
| `shortwin|close-short|hardening-status-unchecked` | LEAD | `positions:2-access-control` | LEAD | close_short accepts a position in any status, so a later ExitInProgress status still lets the borrower close during that exit. No MVP instruction writes that status today. |
| `shortwin|close-short|hardening-status-unchecked` | LEAD | `positions:3-economic-security` | LEAD | close_short accepts a position in any status, so a future ExitInProgress position closes while another exit runs. |
| `shortwin|close-short|hardening-status-unchecked` | LEAD | `positions:4-execution-trace` | LEAD | The close_short instruction accepts a position in any status, so its borrower can close a future ExitInProgress position during another exit. |
| `shortwin|close-short|hardening-status-unchecked` | LEAD | `positions:6-periphery` | LEAD | close_short accepts a position in any status, so its borrower closes a future ExitInProgress position in the middle of that exit. |
| `shortwin|close-short|hardening-status-unchecked` | LEAD | `positions:7-first-principles` | LEAD | close_short does not check position.status, but add_margin does. If a later version writes POSITION_EXIT_IN_PROGRESS, the borrower can close a position during its exit. |
| `shortwin|close-short|hardening-status-unchecked` | LEAD | `positions:8-asymmetry` | LEAD | close_short accepts a position in any status, while add_margin refuses a position that is not Open. A future ExitInProgress status would let the borrower close a position during another exit. |
| `shortwin|close-short|hardening-status-unchecked` | LEAD | `positions:9-account-validation` | LEAD | close_short accepts a ShortPosition in any status, while add_margin requires POSITION_OPEN, so a future ExitInProgress position can still close. |
| `shortwin|close-short|hardening-status-unchecked` | LEAD | `settlement:9-account-validation` | LEAD | close_short never reads position.status, so a future ExitInProgress position still passes the borrower close. |
| `shortwin|deposit|hardening-balance-check-not-delta` | LEAD | `lending:1-math-precision` | LEAD | deposit compares the pool_vault balance with T, so donated tokens hide a transfer fee and the depositor gets shares for tokens the vault never received. The mint filter rejects every fee mint today. |
| `shortwin|deposit|hardening-balance-check-not-delta` | LEAD | `lending:10-numerical-gap` | LEAD | deposit checks that pool_vault holds at least T, not that it received amount, so donated tokens hide a short transfer. The mint filter blocks such mints today. |
| `shortwin|deposit|hardening-balance-check-not-delta` | LEAD | `lending:11-trust-gap` | LEAD | deposit checks the absolute pool_vault balance against T, so untracked tokens in the vault hide a transfer that delivers less than amount. The mint filter blocks such mints today. |
| `shortwin|deposit|hardening-balance-check-not-delta` | LEAD | `lending:12-flow-gap` | LEAD | deposit compares pool_vault.amount with T after the transfer, so donated tokens hide a short transfer. Only the mint filter keeps fee mints out today. |
| `shortwin|deposit|hardening-balance-check-not-delta` | LEAD | `lending:2-access-control` | LEAD | deposit checks that pool_vault.amount is at least T after the transfer, not that the vault received amount, so a donation hides a short transfer. |
| `shortwin|deposit|hardening-balance-check-not-delta` | LEAD | `lending:3-economic-security` | LEAD | deposit checks the absolute pool_vault balance against T, so a donation above T hides a transfer that delivers less than the amount. |
| `shortwin|deposit|hardening-balance-check-not-delta` | LEAD | `lending:4-execution-trace` | LEAD | deposit checks the absolute pool_vault balance against T, so donated tokens hide a transfer that delivers less than amount. The mint filter rejects transfer-fee mints today. |
| `shortwin|deposit|hardening-balance-check-not-delta` | LEAD | `lending:5-invariant` | LEAD | deposit compares the absolute pool vault balance with T, so a token donation to the vault hides a short transfer. The mint filter blocks transfer-fee mints today. |
| `shortwin|deposit|hardening-balance-check-not-delta` | LEAD | `lending:6-periphery` | LEAD | The deposit checks the absolute vault balance, not the amount received, so donated surplus hides a transfer that delivers less than `amount`. No listed mint can do that today. |
| `shortwin|deposit|hardening-balance-check-not-delta` | LEAD | `lending:7-first-principles` | LEAD | deposit compares the vault balance with T, not the received amount, so donated tokens cover a transfer fee. The mint filter rejects every fee mint today. |
| `shortwin|deposit|hardening-balance-check-not-delta` | LEAD | `lending:8-asymmetry` | LEAD | deposit checks that the pool vault holds at least T after the transfer, not that it received amount, so a donation hides a short transfer. |
| `shortwin|deposit|hardening-balance-check-not-delta` | LEAD | `lending:9-account-validation` | LEAD | deposit checks that pool_vault holds at least T after the transfer, not that it received amount, so donations hide a short transfer. We found no mint that passes the filter and transfers less. |
| `shortwin|liquidate|hardening-implied-owner` | LEAD | `settlement:3-economic-security` | LEAD | The exits check `pool_vault` by seeds alone, with no `token::mint` or `token::authority`, so its owner rests on the PDA derivation only. |
| `shortwin|liquidate|hardening-implied-owner` | LEAD | `settlement:4-execution-trace` | LEAD | The exits accept pool_vault by its PDA address alone, with no token::mint or token::authority constraint, so they rely on create_market for both. |
| `shortwin|liquidate|hardening-implied-owner` | LEAD | `settlement:5-invariant` | LEAD | The exits accept pool_vault by its seeds alone, with no token::mint or token::authority constraint, so a future re-creation path would pass unchecked. |
| `shortwin|liquidate|hardening-implied-owner` | LEAD | `settlement:8-asymmetry` | LEAD | pool_vault has no token::mint or token::authority constraint in the exits. Only the PDA address implies its mint and the market authority. |
| `shortwin|liquidate|hardening-implied-owner` | LEAD | `settlement:9-account-validation` | LEAD | liquidate and expire bind pool_vault by seeds alone, so only create_market's derivation proves its mint, authority and token program. |
| `shortwin|liquidate|hardening-status-unchecked` | LEAD | `settlement:3-economic-security` | LEAD | liquidate, expire and sol_settle never read `position.status`, so a v1.1 exit-in-progress position stays open to a second keeper exit. |
| `shortwin|liquidate|hardening-status-unchecked` | LEAD | `settlement:4-execution-trace` | LEAD | liquidate, expire and sol_settle accept a position in any status, while add_margin refuses one that is not open. A v1.1 ExitInProgress status would not stop a second exit. |
| `shortwin|liquidate|hardening-status-unchecked` | LEAD | `settlement:5-invariant` | LEAD | liquidate, expire, sol_settle and close_short never read position.status, so a v1.1 ExitInProgress position can still exit through them. |
| `shortwin|liquidate|hardening-status-unchecked` | LEAD | `settlement:6-periphery` | LEAD | liquidate, expire and sol_settle never read position.status, so a future ExitInProgress position can still go through a second exit. add_margin already checks POSITION_OPEN. |
| `shortwin|liquidate|hardening-status-unchecked` | LEAD | `settlement:7-first-principles` | LEAD | liquidate, expire and sol_settle never read position.status. When v1.1 writes POSITION_EXIT_IN_PROGRESS, a keeper can still exit such a position. |
| `shortwin|liquidate|hardening-status-unchecked` | LEAD | `settlement:8-asymmetry` | LEAD | The exits accept a position in any status, while add_margin requires POSITION_OPEN. A future ExitInProgress status would not stop a second exit. |
| `shortwin|liquidate|hardening-status-unchecked` | LEAD | `settlement:9-account-validation` | LEAD | liquidate, expire and sol_settle never read position.status, so a future ExitInProgress position still passes these exits. |
| `shortwin|liquidate|hardening-version-unchecked` | LEAD | `settlement:3-economic-security` | LEAD | ShortPosition has no layout version that the exits check, so a later layout change lets the exits read new bytes with old offsets. |
| `shortwin|liquidate|hardening-version-unchecked` | LEAD | `settlement:4-execution-trace` | LEAD | ShortPosition carries no layout version, so a later layout change lets the exits read an old position with the new field offsets. |
| `shortwin|liquidate|hardening-version-unchecked` | LEAD | `settlement:5-invariant` | LEAD | ShortPosition carries no layout version, so the exits cannot refuse a position written under an older layout after an upgrade. |
| `shortwin|liquidate|hardening-version-unchecked` | LEAD | `settlement:8-asymmetry` | LEAD | The exits check no layout version of ShortPosition. A future layout change could make an exit read old bytes as new fields. |
| `shortwin|liquidate|hardening-version-unchecked` | LEAD | `settlement:9-account-validation` | LEAD | The exits load ShortPosition, WalletExposure and MarketVault with no layout version check, so after a future change the program reads old bytes as new fields. |
| `shortwin|max-in-liq|carried-position-stale-ert` | LEAD | `settlement:5-invariant` | LEAD | A pump position carried onto a Raydium pool keeps its locked ert, so Raydium exits reserve about 0.0065 SOL that no venue charges. |
| `shortwin|max-in-liq|carried-position-stale-ert` | LEAD | `settlement:9-account-validation` | LEAD | A position opened on the pump curve keeps its ert after a Raydium adoption, so liquidate refuses buybacks that fit and S1 takes them. We did not run that adoption with open positions. |
| `shortwin|open-short|burn-raises-open-fee-cap` | LEAD | `positions:3-economic-security` | LEAD | A holder who burns tokens before an open raises the PumpSwap fee that sizes the fee-scaled caps, so the borrower opens a larger position. We did not measure how far one burn moves the tier. |
| `shortwin|open-short|burn-raises-open-fee-cap` | LEAD | `positions:6-periphery` | LEAD | A holder burns tokens before open_short, so the lower supply moves PumpSwap into a higher fee tier and raises the fee-scaled caps. We did not measure the size gain per burned token. |
| `shortwin|open-short|burn-raises-open-fee-cap` | LEAD | `positions:9-account-validation` | LEAD | A holder who burns tokens before open_short lowers the mint supply, so the PumpSwap fee at P_ema rises and the fee-scaled caps grow. We did not measure the burn size that crosses a pump-fees tier. |
| `shortwin|open-short|hardening-asymmetric-warmup-gate` | LEAD | `positions:1-math-precision` | LEAD | open_short checks ema_warm on Raydium and PumpSwap but not on the curve, so a future curve restart lets opens read an unconfirmed P_ema. |
| `shortwin|open-short|hardening-asymmetric-warmup-gate` | LEAD | `positions:3-economic-security` | LEAD | open_short skips the ema_warm check on curve opens, so a future curve restart lets borrowers open on an unconfirmed P_ema. |
| `shortwin|open-short|hardening-asymmetric-warmup-gate` | LEAD | `positions:4-execution-trace` | LEAD | The open_short instruction checks ema_warm on Raydium and PumpSwap but not on the curve. A future curve restart that sets ema_warm would not stop curve opens. |
| `shortwin|open-short|hardening-asymmetric-warmup-gate` | LEAD | `positions:6-periphery` | LEAD | open_short checks ema_warm only on Raydium and PumpSwap, so a curve market restarted with a positive ema_warm takes opens during its warm-up. No curve path sets ema_warm today. |
| `shortwin|open-short|hardening-asymmetric-warmup-gate` | LEAD | `positions:7-first-principles` | LEAD | open_short checks ema_warm on Raydium and PumpSwap but not on the curve. If a later change marks a curve warm-up, curve opens read a partial EMA seed. |
| `shortwin|open-short|hardening-asymmetric-warmup-gate` | LEAD | `positions:9-account-validation` | LEAD | open_short reads ema_warm on Raydium and PumpSwap only, so a warm-up marked on a curve market does not stop curve opens. |
| `shortwin|open-short|hardening-deny-list-gap` | LEAD | `positions:1-math-precision` | LEAD | The open_short deny list omits trader_base_ata and trader_wsol_ata, so an Emergency extras set can name the trader's own token accounts. |
| `shortwin|open-short|hardening-deny-list-gap` | LEAD | `positions:2-access-control` | LEAD | The open_short deny list omits the trader's token accounts, so an Emergency extras set can add them writable to the trader-signed venue CPI. Only an Emergency write can add them; we found no path for other callers. |
| `shortwin|open-short|hardening-deny-list-gap` | LEAD | `positions:3-economic-security` | LEAD | An Emergency extras set can name the trader's base token account as a writable extra, because the open_short deny list omits it. We did not find a venue that misuses it. |
| `shortwin|open-short|hardening-deny-list-gap` | LEAD | `positions:4-execution-trace` | LEAD | The open_short deny list omits trader_base_ata and trader_wsol_ata, so an Emergency extras set can pass them to the venue again. |
| `shortwin|open-short|hardening-deny-list-gap` | LEAD | `positions:6-periphery` | LEAD | The open_short extras deny list omits the trader's base ATA, its WSOL ATA and its UVA, so an extras set can name those trader accounts. |
| `shortwin|open-short|hardening-deny-list-gap` | LEAD | `positions:7-first-principles` | LEAD | The open_short deny list omits trader_base_ata, trader_wsol_ata and pump_uva. An Emergency extras set can name these trader accounts as trailing accounts. |
| `shortwin|open-short|hardening-deny-list-gap` | LEAD | `positions:9-account-validation` | LEAD | The open_short extras deny list omits trader_base_ata and ATA(trader, WSOL), so an Emergency extras set can name them again as writable venue accounts. |
| `shortwin|open-short|hardening-invariant-unchecked` | LEAD | `positions:1-math-precision` | LEAD | open_short checks only the rent floor when it moves the sell refund, so a refund above the untracked lamports takes tracked position SOL. |
| `shortwin|open-short|hardening-invariant-unchecked` | LEAD | `positions:3-economic-security` | LEAD | open_short moves the sell refund out of the position with only a rent floor, so a future refund error takes collateral that backs v. |
| `shortwin|open-short|hardening-invariant-unchecked` | LEAD | `positions:4-execution-trace` | LEAD | The open_short instruction moves the sell refund with only a rent floor, so a wrong refund can leave the position below rent plus v. |
| `shortwin|open-short|hardening-invariant-unchecked` | LEAD | `positions:6-periphery` | LEAD | open_short moves the sell refund with only a rent floor, so a leg that credits less than S lets the refund spend position SOL. |
| `shortwin|open-short|hardening-invariant-unchecked` | LEAD | `positions:9-account-validation` | LEAD | open_short refunds the sell-leg rent from the position above the rent floor only and never asserts that the position still holds rent plus v. |
| `shortwin|open-short|hardening-stale-mint-supply` | LEAD | `positions:1-math-precision` | LEAD | open_short resolves the post-sale PumpSwap fee tier on the mint supply cached at entry, so it ignores a supply change inside the leg. |
| `shortwin|open-short|hardening-stale-mint-supply` | LEAD | `positions:3-economic-security` | LEAD | open_short prices the post-open PumpSwap state on the mint supply read at entry, so a future leg that burns tokens gets the wrong fee tier. |
| `shortwin|open-short|hardening-stale-mint-supply` | LEAD | `positions:6-periphery` | LEAD | open_short prices the post-open PumpSwap state on the supply read at instruction start, not on the mint account after the leg. No venue changes the supply in the leg today. |
| `shortwin|open-short|hardening-stale-mint-supply` | LEAD | `positions:7-first-principles` | LEAD | open_short reads the post-open venue state with the mint supply from the start of the instruction. A leg that mints or burns gives a wrong PumpSwap fee tier. |
| `shortwin|open-short|hardening-version-unchecked` | LEAD | `positions:1-math-precision` | LEAD | open_short reads the VenueGuard latch without a version check, so a future VenueGuard layout change can make opens read wrong latch bits. |
| `shortwin|open-short|hardening-version-unchecked` | LEAD | `positions:2-access-control` | LEAD | open_short loads VenueGuard without a version check, so a future layout change makes the latch read the wrong bytes. No such layout change exists today. |
| `shortwin|open-short|hardening-version-unchecked` | LEAD | `positions:3-economic-security` | LEAD | open_short reads the VenueGuard latch without its version check, so a future layout change makes the latch read the wrong bytes. |
| `shortwin|open-short|hardening-version-unchecked` | LEAD | `positions:4-execution-trace` | LEAD | The open_short instruction reads the VenueGuard latch without a version check, while flag_venue_upgraded and revalidate_venue check it. A future layout change would make opens read the latch mask at the wrong offset. |
| `shortwin|open-short|hardening-version-unchecked` | LEAD | `positions:6-periphery` | LEAD | open_short loads VenueGuard without its version check, so a future layout change makes opens read the latch at the wrong offsets. |
| `shortwin|open-short|hardening-version-unchecked` | LEAD | `positions:7-first-principles` | LEAD | open_short loads VenueGuard with no check of its version field. After a layout change of VenueGuard, open_short reads the latch bits at the old offsets. |
| `shortwin|open-short|hardening-version-unchecked` | LEAD | `positions:9-account-validation` | LEAD | open_short loads VenueGuard by seeds and bump only and never compares its version with VENUE_GUARD_VERSION, so a later layout change reads the wrong bytes. |
| `shortwin|open-short|margin-fee-view-asymmetry` | LEAD | `positions:3-economic-security` | LEAD | A borrower who trades around an open picks the PumpSwap fee tier of its D4 margin, because open_short quotes the margin at the live fee. We did not measure the saving. |
| `shortwin|open-short|margin-fee-view-asymmetry` | LEAD | `positions:4-execution-trace` | LEAD | The open_short instruction prices N_open and the D4 margin at the post-sale live PumpSwap fee, while the caps use the fee at P_ema. |
| `shortwin|open-short|margin-fee-view-asymmetry` | LEAD | `positions:6-periphery` | LEAD | open_short prices the D4 margin at the live PumpSwap fee, so a buy in the same transaction moves the margin into a lower fee tier. We did not prove that the fee drop outweighs the price rise. |
| `shortwin|open-short|margin-fee-view-asymmetry` | LEAD | `positions:7-first-principles` | LEAD | open_short prices the D4 margin at the live PumpSwap fee of the post-open state, but every exit prices CR at the capped risk fee. We did not prove a loss. |
| `shortwin|open-short|missing-min-rent-bound` | LEAD | `positions:1-math-precision` | LEAD | open_short takes no bound on r_min, so an earlier open that raises utilisation locks a Tier D borrower into 6.75 times the minimum rent. |
| `shortwin|open-short|missing-min-rent-bound` | LEAD | `positions:10-numerical-gap` | LEAD | open_short takes no bound on r_min, so another borrower's earlier open raises a Tier D borrower's minimum rent up to 9.5 times. |
| `shortwin|open-short|missing-min-rent-bound` | LEAD | `positions:3-economic-security` | LEAD | A lender who withdraws free tokens before an open raises the borrower's locked minimum rent, and open_short takes no bound on it. We did not measure a loss beyond the example. |
| `shortwin|open-short|missing-min-rent-bound` | LEAD | `positions:4-execution-trace` | LEAD | The open_short instruction takes no bound on r_min, so a front-run borrow that raises utilisation raises the borrower's locked minimum rent. |
| `shortwin|open-short|missing-min-rent-bound` | LEAD | `positions:6-periphery` | LEAD | open_short takes no bound on the minimum rent, so a borrower whose open lands after a rate change pays a higher r_min than it simulated. |
| `shortwin|open-short|missing-min-rent-bound` | LEAD | `positions:7-first-principles` | LEAD | open_short locks r_min with no bound from the borrower, so a utilisation rise before execution raises the minimum rent that the borrower pays. |
| `shortwin|open-short|missing-min-rent-bound` | LEAD | `positions:9-account-validation` | LEAD | open_short locks r_min at the pre-open utilisation with no borrower bound, so another open just before it raises this borrower's minimum rent. |
| `shortwin|open-short|missing-tier-bound` | LEAD | `positions:1-math-precision` | LEAD | open_short takes no bound on the tier, so a crank between quote and execution can lock the borrower into stricter liquidation and rent terms. |
| `shortwin|open-short|missing-tier-bound` | LEAD | `positions:11-trust-gap` | LEAD | An unlocked LP who withdraws before a borrower's open_short and deposits after it lowers the live R_now, so the position locks a worse tier. The max_margin bound refuses most such cases, because cr_open rises with the tier. |
| `shortwin|open-short|missing-tier-bound` | LEAD | `positions:3-economic-security` | LEAD | Any caller who lands crank_sol_usd or a pool trade before an open changes the tier, so the borrower locks worse terms with no bound. We did not measure a loss. |
| `shortwin|open-short|missing-tier-bound` | LEAD | `positions:4-execution-trace` | LEAD | The open_short instruction takes no bound on the tier it assigns, so a borrower can get stricter liquidation and rent terms than quoted. |
| `shortwin|open-short|missing-tier-bound` | LEAD | `positions:6-periphery` | LEAD | open_short takes no bound on the tier it assigns, so a borrower can lock Tier C terms when it simulated Tier B. |
| `shortwin|open-short|missing-tier-bound` | LEAD | `positions:7-first-principles` | LEAD | open_short takes no bound on the tier that it assigns, so a tier change before execution gives the borrower higher rent rates and an earlier deadline. |
| `shortwin|open-short|missing-tier-bound` | LEAD | `positions:9-account-validation` | LEAD | open_short takes no bound on the tier, so a prior trade that lowers the pool's real SOL gives the borrower Tier C terms. We did not measure how far one trade moves R_eff across a tier threshold. |
| `shortwin|open-short|pumpswap-no-real-sol-floor` | LEAD | `positions:1-math-precision` | LEAD | open_short applies the R_eff ≥ pool_min_real_sol floor on Raydium only, so a borrower opens on a PumpSwap pool whose locked SOL fell below it. |
| `shortwin|open-short|pumpswap-no-real-sol-floor` | LEAD | `positions:3-economic-security` | LEAD | A borrower opens a short on a PumpSwap pool of any depth, because open_short applies the pool_min_real_sol floor to Raydium pools only. We did not prove a loss. |
| `shortwin|open-short|pumpswap-no-real-sol-floor` | LEAD | `positions:4-execution-trace` | LEAD | The open_short instruction applies the pool_min_real_sol floor on Raydium only, so a borrower can open on a thin PumpSwap pool. The source states no intent for the difference. |
| `shortwin|open-short|pumpswap-no-real-sol-floor` | LEAD | `positions:6-periphery` | LEAD | open_short checks R_eff against pool_min_real_sol only on Raydium, so a PumpSwap pool with little real SOL takes opens. |
| `shortwin|open-short|pumpswap-no-real-sol-floor` | LEAD | `positions:7-first-principles` | LEAD | open_short checks R_eff against pool_min_real_sol on Raydium only, so a borrower can open a position on a PumpSwap pool below that floor. |
| `shortwin|open-short|pumpswap-no-real-sol-floor` | LEAD | `positions:9-account-validation` | LEAD | open_short requires R_eff ≥ pool_min_real_sol on Raydium only, so a borrower opens on a PumpSwap pool whose locked real SOL is below that floor. We assume the floor is meant for every pool venue. |
| `shortwin|open-short|stale-ema-open-margin` | FINDING 90 | `positions:5-invariant` | FINDING | open_short accepts a P_ema of any age, so an attacker margins a short at an old low price and keeps the lent tokens. |
| `shortwin|open-short|stale-ema-open-margin` | FINDING 90 | `positions:6-periphery` | FINDING | An attacker pushes the spot down to an idle market's old P_ema, so open_short locks a margin far below the value of the borrowed tokens. |
| `shortwin|open-short|stale-ema-open-margin` | FINDING 90 | `positions:7-first-principles` | LEAD | open_short compares the spot with P_ema, and sizes the D4 EMA leg, with no check of the EMA age. Only the keeper's coverage pause stops opens on an old EMA. |
| `shortwin|open-short|venue-gate-mask-asymmetry` | LEAD | `positions:3-economic-security` | LEAD | The Emergency key disables PumpSwap, but borrowers still open curve shorts that later exit on PumpSwap, because open_static reads only the curve's venue bit. The source does not say whether the PumpSwap bit should stop curve opens. |
| `shortwin|open-short|venue-gate-mask-asymmetry` | LEAD | `positions:4-execution-trace` | LEAD | The open_static guard checks only the curve venue bit on a curve open, so a curve open succeeds while PumpSwap is disabled or route-broken. |
| `shortwin|open-short|venue-gate-mask-asymmetry` | LEAD | `positions:6-periphery` | LEAD | open_static checks only the curve's venue_enabled and route-broken bits on a curve open, so an Emergency PumpSwap disable does not stop curve opens. |
| `shortwin|open-short|venue-gate-mask-asymmetry` | LEAD | `positions:7-first-principles` | LEAD | open_static checks only the curve bit of venue_enabled on a curve open. Curve opens continue while PumpSwap is disabled, and those positions graduate onto PumpSwap. |
| `shortwin|open-short|venue-gate-mask-asymmetry` | LEAD | `positions:9-account-validation` | LEAD | A borrower opens a curve position while PumpSwap is disabled or route-broken, because open_static checks only the curve's venue bits. The curve's latch mask already covers PumpSwap. |
| `shortwin|pump-cashback-claimable|hardening-owner-fail-open` | LEAD | `positions:7-first-principles` | LEAD | pump_cashback_claimable returns 0 for a UVA that pump does not own. If pump moves its UVAs to another owner, sweep_trader sets the reward bit with cashback unclaimed. |
| `shortwin|run|start-sample-pins-warmup-seed` | FINDING 90 | `positions:6-periphery` | FINDING | An attacker pushes the spot down for a PumpSwap price start; run records it as a warm-up bucket, so shorts post margin at that P_ema. |
| `shortwin|sol-settle|fee-view-mismatch` | FINDING 75 | `settlement:3-economic-security` | FINDING | On PumpSwap, sol_settle prices S1 at the fee at `P_ema`, below the live fee that liquidate checks, so no exit can close the position. |
| `shortwin|sol-settle|fee-view-mismatch` | FINDING 75 | `settlement:4-execution-trace` | FINDING | When the live PumpSwap fee exceeds the fee at P_ema, liquidate refuses the buyback and sol_settle refuses S1, so no keeper can exit the position. |
| `shortwin|sol-settle|fee-view-mismatch` | FINDING 75 | `settlement:5-invariant` | FINDING | Correctness: liquidate refuses a PumpSwap buyback at the live fee, and sol_settle tests S1 at the lower risk fee, so no exit runs. |
| `shortwin|sol-settle|fee-view-mismatch` | FINDING 75 | `settlement:7-first-principles` | FINDING | On PumpSwap, sol_settle prices the S1 live leg below the live fee, so no keeper can exit a position that liquidate refuses. |
| `shortwin|sol-settle|fee-view-mismatch` | FINDING 75 | `settlement:8-asymmetry` | FINDING | liquidate tests the buyback at the live fee and sol_settle tests S1 at the capped fee, so neither exit runs until S3 is due. |
| `shortwin|sol-settle|fee-view-mismatch` | FINDING 75 | `settlement:9-account-validation` | FINDING | On PumpSwap, liquidate refuses the buyback at the live fee and sol_settle refuses S1 at the lower P_ema fee, so the position stays open. |
| `shortwin|sol-settle|hardening-conditional-vault-store` | LEAD | `lending:12-flow-gap` | LEAD | The sol_settle no-op stores MarketVault only when lender_sol, unmatched or protocol_fees changed. A future prologue change to the lock state is then lost. |
| `shortwin|sol-settle|hardening-conditional-vault-store` | LEAD | `settlement:3-economic-security` | LEAD | The sol_settle no-op stores MarketVault only when three fields changed, so a future prologue write to another vault field is lost. |
| `shortwin|sol-settle|hardening-conditional-vault-store` | LEAD | `settlement:4-execution-trace` | LEAD | The sol_settle no-op stores MarketVault only when three fields change, so a future prologue change to other vault fields is lost. |
| `shortwin|sol-settle|hardening-conditional-vault-store` | LEAD | `settlement:5-invariant` | LEAD | The sol_settle no-op stores MarketVault only when lender_sol, unmatched or protocol_fees changed, so the instruction drops a future prologue write to any other vault field. |
| `shortwin|sol-settle|hardening-conditional-vault-store` | LEAD | `settlement:6-periphery` | LEAD | The sol_settle no-op stores MarketVault only when one of three fields changed, so it drops any other vault field that a future accrual writes. |
| `shortwin|sol-settle|hardening-conditional-vault-store` | LEAD | `settlement:7-first-principles` | LEAD | The no-op path of sol_settle stores MarketVault only when three fields changed. A future prologue change to another vault field loses that write without an error. |
| `shortwin|sol-settle|hardening-conditional-vault-store` | LEAD | `settlement:8-asymmetry` | LEAD | The sol_settle no-op path stores MarketVault only when three fields change. A later prologue write to another vault field would be lost. |
| `shortwin|sol-settle|hardening-conditional-vault-store` | LEAD | `settlement:9-account-validation` | LEAD | The sol_settle no-op stores MarketVault only when three fields changed, so the no-op drops a future accrual's writes to other vault fields. |
| `shortwin|sol-settle|hardening-vacuous-token-program-check` | LEAD | `lending:12-flow-gap` | LEAD | sol_settle passes market.token_program to mint_guard as the token program, so that key check compares a value with itself. The mint owner check still binds the mint. |
| `shortwin|sol-settle|hardening-vacuous-token-program-check` | LEAD | `settlement:3-economic-security` | LEAD | sol_settle passes `market.token_program` to `mint_guard` as the token program, so the guard's third key check always passes. A later edit that drops the owner check would leave no token-program check. |
| `shortwin|sol-settle|hardening-vacuous-token-program-check` | LEAD | `settlement:4-execution-trace` | LEAD | sol_settle passes market.token_program as the token program to mint_guard, so its third key check always passes. |
| `shortwin|sol-settle|hardening-vacuous-token-program-check` | LEAD | `settlement:5-invariant` | LEAD | sol_settle passes market.token_program to mint_guard as the token program, so the guard compares that key with itself. A future token leg here would trust an unchecked program. |
| `shortwin|sol-settle|hardening-vacuous-token-program-check` | LEAD | `settlement:6-periphery` | LEAD | sol_settle passes market.token_program to mint_guard as the token program, so its third key check compares the field with itself. The owner check still binds the mint today. |
| `shortwin|sol-settle|hardening-vacuous-token-program-check` | LEAD | `settlement:7-first-principles` | LEAD | sol_settle passes market.token_program to mint_guard as the token program, so that key check compares a key with itself. If a later change adds a token CPI here, the guard does not prove which program the CPI uses. |
| `shortwin|sol-settle|hardening-vacuous-token-program-check` | LEAD | `settlement:8-asymmetry` | LEAD | sol_settle passes market.token_program to mint_guard as the token program, so that check is always true. A later token CPI here would rely on a check that does nothing. |
| `shortwin|sol-settle|hardening-vacuous-token-program-check` | LEAD | `settlement:9-account-validation` | LEAD | sol_settle passes market.token_program to mint_guard as the token program, so the guard's third key check compares a value with itself. |
| `shortwin|sol-settle|risk-fee-tier-lowered-by-same-tx-buy` | LEAD | `settlement:3-economic-security` | LEAD | A buy earlier in the transaction lowers the PumpSwap fee tier, so sol_settle pays lenders an S2 or S3 amount at that lower fee. We did not measure the tier step on a live pool: KI-72 gives 2–5 bps, and up to 70 bps at one boundary. |
| `shortwin|sol-settle|risk-fee-tier-lowered-by-same-tx-buy` | LEAD | `settlement:4-execution-trace` | LEAD | A borrower buys before sol_settle in one transaction, so the PumpSwap fee tier drops and the S2 or S3 fair value pays lenders less. |
| `shortwin|sol-settle|risk-fee-tier-lowered-by-same-tx-buy` | LEAD | `settlement:5-invariant` | LEAD | A borrower who calls sol_settle as keeper can buy first in the same transaction, lower the live PumpSwap fee tier and reduce the lenders' FV. We did not measure the round-trip cost against the FV reduction. |
| `shortwin|sol-settle|risk-fee-tier-lowered-by-same-tx-buy` | LEAD | `settlement:7-first-principles` | LEAD | A caller who buys before sol_settle in one transaction lowers the PumpSwap fee tier, so S2 and S3 pay the lenders a lower FV. We did not measure the round-trip cost of the buy against the gain of up to 0.7 % of B̂. |
| `shortwin|sol-settle|risk-fee-tier-lowered-by-same-tx-buy` | LEAD | `settlement:8-asymmetry` | LEAD | A buy before sol_settle lowers the PumpSwap live fee tier, so the borrower can lower FV or refute an armed clock. We did not size the gain against the swap fees. |
| `shortwin|sol-settle|risk-fee-tier-lowered-by-same-tx-buy` | LEAD | `settlement:9-account-validation` | LEAD | A caller who buys on PumpSwap before sol_settle lowers the live fee tier, so the S2 or S3 fair value and the lenders' take fall. We did not measure whether the saved fee pays for the round trip. |
| `shortwin|sol-settle|s3-wait-from-market-chain` | LEAD | `settlement:4-execution-trace` | LEAD | sol_settle times S3 from the market chain at the lowest locked cap, so a higher-cap position settles seconds after its own no-route begins. |
| `shortwin|sol-settle|s3-wait-from-market-chain` | LEAD | `settlement:5-invariant` | LEAD | sol_settle starts a position's S3 wait at the market chain, which a lower locked cap started, so the position waits less than its s3_wait_s. |
| `shortwin|sol-settle|s3-wait-from-market-chain` | LEAD | `settlement:8-asymmetry` | LEAD | sol_settle times a position's S3 wait from the market chain, which a lower cap starts earlier. The position can reach S3 seconds after its own cap fails. |
| `shortwin|sol-settle|s3-wait-from-market-chain` | LEAD | `settlement:9-account-validation` | LEAD | sol_settle times each S3 wait from the market chain, so a position with a higher fee cap reaches S3 soon after its own route fails. We did not prove a loss beyond the shorter wait. |
| `shortwin|sol-settle|vested-principal-diluted-by-late-deposit` | FINDING 90 | `lending:1-math-precision` | LEAD | A depositor who joins after a halted-venue S3 and waits lump_vest_s takes most of the token value that vests for the old lenders. KI-24 accepts that a holder earns its share; we assume the owner did not weigh a deposit that buys most of S after the loss. |
| `shortwin|sol-settle|vested-principal-diluted-by-late-deposit` | FINDING 90 | `lending:12-flow-gap` | FINDING | After a halted-venue S3 the whole take vests, so a depositor who enters next takes most of the old lenders' SOL for their tokens. |
| `shortwin|sol-settle|vested-principal-diluted-by-late-deposit` | FINDING 90 | `lending:6-periphery` | FINDING | A depositor who enters after an S3 on a halted venue takes most of the vested SOL that pays the old lenders for their tokens. |
| `shortwin|sol-settle|vested-principal-diluted-by-late-deposit` | FINDING 90 | `lending:8-asymmetry` | FINDING | A depositor after a halted-venue S3 settlement receives most of the old lenders' token value, because that value vests and deposit stays open. |
| `shortwin|sol-settle|vested-principal-diluted-by-late-deposit` | FINDING 90 | `lending:9-account-validation` | FINDING | After a settlement leaves more than 1/1,000 of the claims, a later depositor holds most shares and takes the old lenders' SOL that still vests. |
| `shortwin|sol-settle|vested-principal-diluted-by-late-deposit` | FINDING 90 | `settlement:10-numerical-gap` | FINDING | A depositor who enters after a halted-venue S3 takes most of its vested SOL, which pays the old lenders for their lost tokens. |
| `shortwin|sol-settle|vested-principal-diluted-by-late-deposit` | FINDING 90 | `settlement:3-economic-security` | FINDING | After an S3 on a halted venue, a depositor buys shares at the token price alone and takes part of the old lenders' unvested SOL. |
| `shortwin|sol-settle|vested-principal-diluted-by-late-deposit` | FINDING 90 | `settlement:4-execution-trace` | FINDING | After an S3 on a halted venue, a later depositor takes part of the vested SOL that pays old lenders for their extinguished tokens. |
| `shortwin|sol-settle|vested-principal-diluted-by-late-deposit` | FINDING 90 | `settlement:5-invariant` | FINDING | After an S3 on a halted venue, a new depositor takes a pro-rata share of the vest that repays the old lenders' extinguished tokens. |
| `shortwin|sol-settle|vested-principal-diluted-by-late-deposit` | FINDING 90 | `settlement:6-periphery` | FINDING | A depositor who joins after a halted-venue S3 takes part of the old lenders' settlement SOL, because sol_settle vests the whole principal. |
| `shortwin|sol-settle|vested-principal-diluted-by-late-deposit` | FINDING 90 | `settlement:8-asymmetry` | FINDING | A depositor who joins after an S3 on a halted venue takes part of the old lenders' vested token compensation. |
| `shortwin|sol-settle|vested-principal-diluted-by-late-deposit` | FINDING 90 | `settlement:9-account-validation` | FINDING | A lender who deposits after a halted-venue settlement receives part of the old lenders' vested principal, because deposits price shares on tokens only. |
| `shortwin|store-records|hardening-unvalidated-writer` | LEAD | `lending:1-math-precision` | LEAD | store_records writes conv_rem into the accounts it receives and checks only is_writable. A future caller that passes another slice overwrites 16 bytes of a program account. |
| `shortwin|store-records|hardening-unvalidated-writer` | LEAD | `lending:10-numerical-gap` | LEAD | store_records writes conv_rem into each account without a new owner or key check, and trusts that load_records checked the same slice in the same order. A caller that passes another slice writes 16 bytes into an unchecked account. |
| `shortwin|store-records|hardening-unvalidated-writer` | LEAD | `lending:11-trust-gap` | LEAD | store_records writes 16 bytes into each passed account with no owner or address check; it relies on callers to pass the slice load_records checked. |
| `shortwin|store-records|hardening-unvalidated-writer` | LEAD | `lending:12-flow-gap` | LEAD | store_records writes conv_rem into bytes 127..143 of each changed account and checks only is_writable. A future caller that passes another slice overwrites program-owned data. |
| `shortwin|store-records|hardening-unvalidated-writer` | LEAD | `lending:2-access-control` | LEAD | store_records writes conv_rem into each changed account with no owner or key check, so it trusts the caller to pass the accounts load_records checked. |
| `shortwin|store-records|hardening-unvalidated-writer` | LEAD | `lending:3-economic-security` | LEAD | store_records writes 16 bytes into each changed account and trusts that load_records checked the same slice, so a future caller mismatch corrupts another account. |
| `shortwin|store-records|hardening-unvalidated-writer` | LEAD | `lending:4-execution-trace` | LEAD | store_records writes 16 bytes into the remaining accounts at the indices load_records checked, and does not check them again itself. A future caller that passes another slice would write into accounts nobody checked. |
| `shortwin|store-records|hardening-unvalidated-writer` | LEAD | `lending:5-invariant` | LEAD | store_records writes 16 bytes into each changed account and checks only is_writable. A caller that passes it other accounts than load_records read writes unchecked data. |
| `shortwin|store-records|hardening-unvalidated-writer` | LEAD | `lending:6-periphery` | LEAD | store_records writes 16 bytes at offset 127 of each changed record without a re-check, so a caller with another slice corrupts that account. All callers pass the same slice today. |
| `shortwin|store-records|hardening-unvalidated-writer` | LEAD | `lending:7-first-principles` | LEAD | store_records writes 16 bytes at offset 127 of each changed account and checks only that it is writable. A future caller that passes another slice overwrites a program-owned account. |
| `shortwin|store-records|hardening-unvalidated-writer` | LEAD | `lending:8-asymmetry` | LEAD | store_records writes 16 bytes at a fixed offset into each changed record and relies on load_records alone for the owner and discriminator checks. |
| `shortwin|store-records|hardening-unvalidated-writer` | LEAD | `lending:9-account-validation` | LEAD | store_records writes 16 bytes into each changed remaining account and relies on load_records for every owner, type and address check. A future caller that skips load_records writes into unchecked accounts. |
| `shortwin|sweep-trader|cashback-destination-not-swept` | FINDING 75 | `positions:12-flow-gap` | FINDING | Any caller can claim the PumpSwap cashback of the trader into a WSOL account that sweep_trader never closes, so the lenders lose that cashback. |
| `shortwin|sweep-trader|cashback-destination-not-swept` | FINDING 75 | `positions:2-access-control` | FINDING | Any caller can move the trader's PumpSwap cashback into a non-ATA WSOL account of the trader, which no instruction sweeps, so lenders lose it. |
| `shortwin|sweep-trader|cashback-destination-not-swept` | FINDING 75 | `positions:3-economic-security` | FINDING | Any caller sends the trader's PumpSwap cashback to a trader-owned WSOL account that sweep_trader never closes, so lenders never receive it. |
| `shortwin|sweep-trader|cashback-destination-not-swept` | FINDING 75 | `positions:6-periphery` | FINDING | Any caller claims the trader's PumpSwap cashback into a new WSOL account that sweep_trader never closes, so the lenders never receive it. |
| `shortwin|sweep-trader|cashback-destination-not-swept` | FINDING 75 | `positions:8-asymmetry` | FINDING | Any caller can claim the trader's PumpSwap cashback into a WSOL account that no short.win instruction closes, so lenders never receive that cashback. |
| `shortwin|sweep-trader|cashback-destination-not-swept` | FINDING 75 | `positions:9-account-validation` | FINDING | Any caller can claim the trader's PumpSwap cashback into a non-ATA WSOL account, which sweep_trader never closes, so lenders lose that cashback. |
| `shortwin|sweep-trader|foreign-claim-dependency` | LEAD | `positions:12-flow-gap` | LEAD | If pAMM makes claim_cashback require the trader's signature, a WSOL donation to ATA(pAMM UVA(trader), WSOL) blocks every deposit after a full wipe. We did not prove such a pAMM change. |
| `shortwin|sweep-trader|foreign-claim-dependency` | LEAD | `positions:2-access-control` | LEAD | An attacker can donate 1 lamport to the trader's pump UVA after a full wipe, so deposits stay blocked until someone sends the venue claim. The block becomes permanent only if pump or pAMM makes the trader a required signer of claim_cashback, which we did not prove. |
| `shortwin|sweep-trader|foreign-claim-dependency` | LEAD | `positions:3-economic-security` | LEAD | Any caller who sends lamports to the trader's pump UVA blocks post-wipe deposits until pump's claim_cashback pays them out. We did not prove that a pump upgrade makes claim_cashback need the trader's signature. |
| `shortwin|sweep-trader|foreign-claim-dependency` | LEAD | `positions:6-periphery` | LEAD | After a full wipe, deposits wait until sweep_trader sees no venue cashback, so a change of pump's or pAMM's claim_cashback blocks every deposit. We did not prove that either venue will change it. |
| `shortwin|sweep-trader|foreign-claim-dependency` | LEAD | `positions:7-first-principles` | LEAD | After a full wipe, deposits wait until pump's claim_cashback empties the trader's UVA. If pump ever requires the trader's signature, no deposit lands on that market again. |
| `shortwin|sweep-trader|foreign-claim-dependency` | LEAD | `positions:9-account-validation` | LEAD | sweep_trader treats every pump UVA lamport above rent as cashback, so a donation blocks post-wipe deposits once claim_cashback pays only cashback_earned. The pinned pump ELF pays every lamport above rent today (T-CASHBACK), and we did not prove a pump change. |
| `shortwin|sync-lender|hardening-raw-clock-lock-calendar` | LEAD | `lending:1-math-precision` | LEAD | Any caller's sync_lender moves the lock calendar at the raw clock, so after a clock step back a lock holder's withdraw fails with InvalidInput. Agave's clock does not step back today. |
| `shortwin|sync-lender|hardening-raw-clock-lock-calendar` | LEAD | `lending:10-numerical-gap` | LEAD | Any caller's sync_lender advances the lock calendar at the raw clock, not market_now. A clock step back of more than lock_grace_s then makes a withdraw that adds a lock fail. |
| `shortwin|sync-lender|hardening-raw-clock-lock-calendar` | LEAD | `lending:11-trust-gap` | LEAD | Any caller's sync_lender moves the lock calendar at the raw clock, while deposit and withdraw use market_now. We found no wrong value today; a later time-dependent read there uses the other time base. |
| `shortwin|sync-lender|hardening-raw-clock-lock-calendar` | LEAD | `lending:12-flow-gap` | LEAD | Any caller's sync_lender advances the lock calendar at the raw clock, so after a clock step back a lock holder's withdraw fails with InvalidParams. Agave prevents the step back today. |
| `shortwin|sync-lender|hardening-raw-clock-lock-calendar` | LEAD | `lending:2-access-control` | LEAD | Any caller's sync_lender advances the MarketVault lock calendar at the raw clock, not at market_now. We found no path today where the two times differ. |
| `shortwin|sync-lender|hardening-raw-clock-lock-calendar` | LEAD | `lending:3-economic-security` | LEAD | Any caller's sync_lender advances the lock calendar at the raw clock, not at market_now that the Market-writing instructions use. |
| `shortwin|sync-lender|hardening-raw-clock-lock-calendar` | LEAD | `lending:4-execution-trace` | LEAD | Any caller's sync_lender advances the lock calendar at the raw clock, so a later withdraw at market_now can see a lock its aggregate dropped. No path moves value today. |
| `shortwin|sync-lender|hardening-raw-clock-lock-calendar` | LEAD | `lending:5-invariant` | LEAD | Any caller's sync_lender advances the lock calendar and tests lock expiry at the raw clock. A later rule that reads the calendar would see a time before the last accrual. |
| `shortwin|sync-lender|hardening-raw-clock-lock-calendar` | LEAD | `lending:6-periphery` | LEAD | Any caller's sync_lender advances the lock calendar and clears expired locks at the raw clock, not at `market_now`, unlike deposit and withdraw. No caller gains value today. |
| `shortwin|sync-lender|hardening-raw-clock-lock-calendar` | LEAD | `lending:7-first-principles` | LEAD | Any caller's sync_lender advances the lock calendar at the raw clock. After a clock step back, a lock holder's later withdraw at market_now can fail in cal_add. |
| `shortwin|sync-lender|hardening-raw-clock-lock-calendar` | LEAD | `lending:8-asymmetry` | LEAD | Any caller's sync_lender advances the lock calendar at the raw clock, which can be later than the time the next withdraw uses. We did not prove a loss. |
| `shortwin|sync-lender|hardening-raw-clock-lock-calendar` | LEAD | `lending:9-account-validation` | LEAD | Any caller's sync_lender advances the lock calendar of MarketVault at the raw clock, with no signer and no Market accrual. |
| `shortwin|sync|s3-timer-missing-grace` | LEAD | `settlement:8-asymmetry` | LEAD | sync moves a migrating market to None without graduated_at, so sol_settle can S3-settle its positions in the same call. No grace period gives a Raydium pool time to arrive. |
| `shortwin|withdraw|hardening-same-account-legs` | LEAD | `lending:10-numerical-gap` | LEAD | withdraw takes any token account of the mint as the destination, and only Anchor's duplicate check refuses pool_vault. Without that check, the lender burns shares for nothing. |
| `shortwin|withdraw|hardening-same-account-legs` | LEAD | `lending:11-trust-gap` | LEAD | withdraw has no check that owner_token_account differs from pool_vault; only Anchor's duplicate-account check stops a self-transfer that takes the lender's shares and moves no tokens. |
| `shortwin|withdraw|hardening-same-account-legs` | LEAD | `lending:12-flow-gap` | LEAD | Only the Anchor 1.2.0 duplicate-mutable check stops a lender who passes pool_vault as owner_token_account; without it the lender burns shares and receives nothing. |
| `shortwin|withdraw|hardening-same-account-legs` | LEAD | `lending:2-access-control` | LEAD | withdraw has no explicit check that owner_token_account differs from pool_vault, so only Anchor's generated duplicate check refuses a self-transfer. |
| `shortwin|withdraw|hardening-same-account-legs` | LEAD | `lending:3-economic-security` | LEAD | Only Anchor's duplicate-account check stops pool_vault as the withdraw destination, and the token program accepts a transfer to the same account. |
| `shortwin|withdraw|hardening-same-account-legs` | LEAD | `lending:4-execution-trace` | LEAD | withdraw relies only on Anchor's generated duplicate check to refuse pool_vault as owner_token_account; no constraint states it. A move to an UncheckedAccount field or an older Anchor would drop that check. |
| `shortwin|withdraw|hardening-same-account-legs` | LEAD | `lending:5-invariant` | LEAD | Only Anchor's duplicate-account check stops owner_token_account equal to pool_vault. Without that check, a self-transfer moves no tokens and T still falls. |
| `shortwin|withdraw|hardening-same-account-legs` | LEAD | `lending:6-periphery` | LEAD | Only Anchor's duplicate-account check refuses `pool_vault` as the withdraw destination. A future account-type change makes the transfer a self-transfer that pays the lender nothing. |
| `shortwin|withdraw|hardening-same-account-legs` | LEAD | `lending:7-first-principles` | LEAD | Only Anchor's duplicate-account check refuses pool_vault as the withdraw destination. If a later change removes that check, the lender loses its burned shares and the tokens stay in the vault. |
| `shortwin|withdraw|hardening-same-account-legs` | LEAD | `lending:8-asymmetry` | LEAD | Only Anchor's duplicate mutable account check stops a withdraw that names the pool vault as its own destination. |
| `shortwin|withdraw|hardening-same-account-legs` | LEAD | `lending:9-account-validation` | LEAD | Only Anchor's duplicate-account check refuses pool_vault as owner_token_account in withdraw, because the handler has no key check of its own. |
