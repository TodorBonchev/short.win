# 🔐 Security Review — shortwin

---

## Scope

|  |  |
| --- | --- |
| **Mode** | filename |
| **Files reviewed** | `./crates/shortwin-math/src/bounds.rs` · `./crates/shortwin-math/src/caps.rs` · `./crates/shortwin-math/src/consts.rs`<br>`./crates/shortwin-math/src/error.rs` · `./crates/shortwin-math/src/fees.rs` · `./crates/shortwin-math/src/lib.rs`<br>`./crates/shortwin-math/src/num/mod.rs` · `./crates/shortwin-math/src/num/u256.rs` · `./crates/shortwin-math/src/observe.rs`<br>`./crates/shortwin-math/src/params.rs` · `./crates/shortwin-math/src/persist.rs` · `./crates/shortwin-math/src/pool.rs`<br>`./crates/shortwin-math/src/pool_select.rs` · `./crates/shortwin-math/src/quote/cpmm.rs` · `./crates/shortwin-math/src/quote/mod.rs`<br>`./crates/shortwin-math/src/quote/pricing.rs` · `./crates/shortwin-math/src/quote/ray_v4.rs` · `./crates/shortwin-math/src/quote/reprice.rs`<br>`./crates/shortwin-math/src/quote/sol_leg.rs` · `./crates/shortwin-math/src/quote/straddle.rs` · `./crates/shortwin-math/src/rent.rs`<br>`./crates/shortwin-math/src/risk.rs` · `./crates/shortwin-math/src/settle.rs` · `./crates/shortwin-math/src/tier.rs`<br>`./crates/shortwin-math/src/waterfall.rs` · `./crates/shortwin-venue-state/src/bytes.rs` · `./crates/shortwin-venue-state/src/lib.rs`<br>`./crates/shortwin-venue-state/src/mint.rs` · `./crates/shortwin-venue-state/src/mock_cpamm.rs` · `./crates/shortwin-venue-state/src/program_data.rs`<br>`./crates/shortwin-venue-state/src/pump_amm.rs` · `./crates/shortwin-venue-state/src/pump.rs` · `./crates/shortwin-venue-state/src/pyth.rs`<br>`./crates/shortwin-venue-state/src/raydium_cpmm.rs` · `./crates/shortwin-venue-state/src/raydium_v4.rs` · `./crates/shortwin-venue-state/src/token_account.rs`<br>`./programs/shortwin/src/constants.rs` · `./programs/shortwin/src/engine/arming.rs` · `./programs/shortwin/src/engine/bounds.rs`<br>`./programs/shortwin/src/engine/caps.rs` · `./programs/shortwin/src/engine/guard.rs` · `./programs/shortwin/src/engine/lender.rs`<br>`./programs/shortwin/src/engine/mod.rs` · `./programs/shortwin/src/engine/observe.rs` · `./programs/shortwin/src/engine/pool_select.rs`<br>`./programs/shortwin/src/engine/prologue.rs` · `./programs/shortwin/src/engine/route.rs` · `./programs/shortwin/src/engine/tier.rs`<br>`./programs/shortwin/src/engine/trader.rs` · `./programs/shortwin/src/engine/waterfall.rs` · `./programs/shortwin/src/errors.rs`<br>`./programs/shortwin/src/events.rs` · `./programs/shortwin/src/instructions/add_margin.rs` · `./programs/shortwin/src/instructions/apply_config.rs`<br>`./programs/shortwin/src/instructions/cancel_config.rs` · `./programs/shortwin/src/instructions/claim_sol.rs` · `./programs/shortwin/src/instructions/close_short.rs`<br>`./programs/shortwin/src/instructions/cover_deficit.rs` · `./programs/shortwin/src/instructions/crank_price.rs` · `./programs/shortwin/src/instructions/crank_sol_usd.rs`<br>`./programs/shortwin/src/instructions/create_market.rs` · `./programs/shortwin/src/instructions/deposit.rs` · `./programs/shortwin/src/instructions/disable_venue.rs`<br>`./programs/shortwin/src/instructions/execute_pool_switch.rs` · `./programs/shortwin/src/instructions/expire.rs` · `./programs/shortwin/src/instructions/flag_unsafe_mint.rs`<br>`./programs/shortwin/src/instructions/flag_venue_upgraded.rs` · `./programs/shortwin/src/instructions/initialize_config.rs` · `./programs/shortwin/src/instructions/liquidate.rs`<br>`./programs/shortwin/src/instructions/mod.rs` · `./programs/shortwin/src/instructions/open_short.rs` · `./programs/shortwin/src/instructions/propose_config.rs`<br>`./programs/shortwin/src/instructions/propose_pool.rs` · `./programs/shortwin/src/instructions/revalidate_venue.rs` · `./programs/shortwin/src/instructions/set_coverage_pause.rs`<br>`./programs/shortwin/src/instructions/set_pause.rs` · `./programs/shortwin/src/instructions/set_route_broken.rs` · `./programs/shortwin/src/instructions/set_venue_extras.rs`<br>`./programs/shortwin/src/instructions/sol_settle.rs` · `./programs/shortwin/src/instructions/sweep_protocol_fees.rs` · `./programs/shortwin/src/instructions/sweep_trader.rs`<br>`./programs/shortwin/src/instructions/sync_lender.rs` · `./programs/shortwin/src/instructions/withdraw.rs` · `./programs/shortwin/src/lamports.rs`<br>`./programs/shortwin/src/lib.rs` · `./programs/shortwin/src/mint_filter.rs` · `./programs/shortwin/src/state/config.rs`<br>`./programs/shortwin/src/state/epoch.rs` · `./programs/shortwin/src/state/exposure.rs` · `./programs/shortwin/src/state/layout_asserts.rs`<br>`./programs/shortwin/src/state/lender.rs` · `./programs/shortwin/src/state/market.rs` · `./programs/shortwin/src/state/mod.rs`<br>`./programs/shortwin/src/state/oracle.rs` · `./programs/shortwin/src/state/pod.rs` · `./programs/shortwin/src/state/position.rs`<br>`./programs/shortwin/src/state/vault.rs` · `./programs/shortwin/src/state/venue_extras.rs` · `./programs/shortwin/src/state/venue_guard.rs`<br>`./programs/shortwin/src/venues/detect.rs` · `./programs/shortwin/src/venues/mock.rs` · `./programs/shortwin/src/venues/mod.rs`<br>`./programs/shortwin/src/venues/pump_amm.rs` · `./programs/shortwin/src/venues/pump_curve.rs` · `./programs/shortwin/src/venues/raydium_cpmm.rs`<br>`./programs/shortwin/src/venues/raydium.rs` · `./programs/shortwin/src/venues/raydium_v4.rs` · `./services/keeper/src/bin/build_alt.rs`<br>`./services/keeper/src/bin/v1_probe.rs` |
| **Confidence threshold (1-100)** | 75 |
| **Passes** | 1 of 3 (pass 2 failed) |
| **Memory** | 0 records before this scan · 98 after · `e035f2e` |

---

## Findings

[90] **1. A rebase gives a locked lender a share worth all free tokens**

`shortwin-math::convert_steps` · Confidence: 90 · NEW · PoC: UNVERIFIED

**Description**
A rebase floors a locked lender's lock but rounds its shares up, so the lender withdraws all free tokens that the others should share.

**Fix**

```diff
--- a/crates/shortwin-math/src/pool.rs
+++ b/crates/shortwin-math/src/pool.rs
@@ -1243,6 +1243,8 @@ fn convert_steps<R: Recs>(
+        // ECON 2.1 (module docs): a lender's own lock rounds up; floor only its unlocked part.
+        let unlocked = scale(sub_u128(l.shares, l.lock.min(l.shares))?)?;
         let (shares, rem) = convert_shares(l.shares, rec.conv_rem, num, den)?;
         if rem != rec.conv_rem {
             records.set_rem(idx, rem)?;
         }
         l.shares = shares;
-        l.lock = scale(l.lock)?;
+        l.lock = sub_u128(shares, unlocked.min(shares))?;
```

---

[90] **2. open_short does not check the age of P_ema.**

`shortwin::open_short` · Confidence: 90 · NEW · PoC: CONFIRMED

**Description**
An attacker sells the pool down to an unobserved market's old P_ema, opens shorts there, and keeps lender tokens worth more than the margin.

**Fix**

```diff
--- a/programs/shortwin/src/instructions/open_short.rs
+++ b/programs/shortwin/src/instructions/open_short.rs
@@ -340,6 +340,13 @@
             .checked_add(i64::from(config.params.market_warmup_s))
             .or_math()?;
         require_gte!(now, warm, ShortwinError::MarketWarmingUp);
+        // The D4 margin, the caps and the spot/EMA guard read P_ema: an EMA that integrated
+        // nothing for COVERAGE_GAP_S is an old price, not the pool's (an idle market is not
+        // observed), so the open waits for fresh observations.
+        require!(
+            now.saturating_sub(market.ema_ts) <= shortwin_math::consts::COVERAGE_GAP_S,
+            ShortwinError::EmaWarmingUp
+        );
         let p_ema = market.ema_price.get();
         let p_spot = state.price().ok_or(ShortwinError::NoRoute)?;
         require!(
```

---

[90] **3. The PumpSwap price start keeps P_ema at a pushed-down spot**

`shortwin::run` · Confidence: 90 · NEW · PoC: CONFIRMED

**Description**
The prologue records the start spot as a warm-up bucket, so an attacker who pushes the start down opens under-margined shorts at the lenders' cost.

**Fix**

```diff
--- a/programs/shortwin/src/engine/prologue.rs
+++ b/programs/shortwin/src/engine/prologue.rs
@@ -174,10 +174,13 @@
             // A PumpSwap market without a price starts it at its first executable observation,
             // marked for the warm-up (a PumpSwap listing or re-detection, M6), its liquidity
             // capped at the pool's locked depth `D` (M6R-1; over `λ_min` = 100 %, OWNER-M6D-1).
-            if market.venue_kind() == Some(VenueKind::PumpSwap) {
-                pump_amm::start_price(&mut obs, st, now)?;
-            }
-            recorded = observe::record(&mut obs, now, st)?;
+            let started = market.venue_kind() == Some(VenueKind::PumpSwap)
+                && pump_amm::start_price(&mut obs, st, now)?;
+            // The start's own sample is not a warm-up bucket: the warm-up's running minimum
+            // would hold P_ema at a pushed start for all eight integrations.
+            if !started {
+                recorded = observe::record(&mut obs, now, st)?;
+            }
             let chains = route::observe_chains(config, market, st, now);
```

---

[90] **4. A deposit after a halted-venue S3 takes the old lenders' vested SOL**

`shortwin::sol_settle` · Confidence: 90 · NEW · PoC: CONFIRMED

**Description**
sol_settle vests a halted-venue S3, so a later depositor takes part of the old lenders' SOL; also in settle, after a deposit stops the flush.

**Fix**

```diff
--- a/crates/shortwin-math/src/pool.rs
+++ b/crates/shortwin-math/src/pool.rs
@@
 pub const VEST_FLUSH_RATIO: u128 = 1_000;
+/// A settlement that leaves at most 1/VEST_HOLD_RATIO of the claims without a flush holds
+/// deposits until `vest_end` (the program stores and checks it).
+pub const VEST_HOLD_RATIO: u128 = 2;
@@ pub struct Settled {
     pub flushed: bool,
+    pub hold: bool,
 }
@@ pub fn settle(
     if flushed {
         flush_vest_in(&mut p, now)?;
     }
+    let hold = !flushed
+        && p.vest_bal != 0
+        && ovf(p.token_claims()?.checked_mul(VEST_HOLD_RATIO))? <= claims;
     let rec = maybe_roll_in(&mut p, &mut v, now)?;
@@
     Ok(Settled {
         rec,
         vested,
         flushed,
+        hold,
     })
--- a/programs/shortwin/src/state/market.rs
+++ b/programs/shortwin/src/state/market.rs
@@
     pub min_open_hard_cap_bps: u16,
     pub _pad6: [u8; 6],
-    pub _reserved: [u8; 144],
+    /// No deposit before this second: the vest of a settlement whose token part vested.
+    pub deposit_hold_until: i64,
+    pub _reserved: [u8; 136],
 }
--- a/programs/shortwin/src/instructions/sol_settle.rs
+++ b/programs/shortwin/src/instructions/sol_settle.rs
@@
             let mut aggr = market.aggr();
             engine_waterfall::remove_position(&mut market, &mut aggr, pos)?;
             market.set_aggr(&aggr);
+            // AIR-09 vested the token part of the take: only the shares that held the
+            // extinguished tokens may receive it.
+            if !inp.executable && !settled.flushed && pool.vest_bal != 0 {
+                market.deposit_hold_until = market.deposit_hold_until.max(pool.vest_end);
+            }
+            if settled.hold {
+                market.deposit_hold_until = market.deposit_hold_until.max(pool.vest_end);
+            }
             market.set_pool(&pool);
--- a/programs/shortwin/src/instructions/deposit.rs
+++ b/programs/shortwin/src/instructions/deposit.rs
@@
         let now = prologue::market_now(&market, clock.unix_timestamp);
+        require!(now >= market.deposit_hold_until, ShortwinError::PoolDrained);
 
         prologue::sync_market_params(&config, &mut market, now);
```
(layout_asserts.rs needs the new offset @2960)

---

[80] **5. After a dust flush a depositor takes the pre-wipe cashback**

`shortwin-math::settle` · Confidence: 80 · NEW

**Description**
After a dust flush leaves shares, a deposit does not wait for the post-wipe sweeps, so a dust depositor takes the old lenders' cashback.

**Fix**

```diff
--- a/crates/shortwin-math/src/pool.rs
+++ b/crates/shortwin-math/src/pool.rs
@@ -1980,19 +1980,17 @@
         // deposit is what brings `S` back (ARCH §9 M0b note 4), so `S = 0` is not drained.
         return Err(MathError::PoolDrained);
     }
+    let need = if rewards_required {
+        WIPE_TRADER_SWEPT | WIPE_REWARDS_CLAIMED
+    } else {
+        WIPE_TRADER_SWEPT
+    };
+    if p.wipe_swept & need != need {
+        // R4A-7, AIR-08 and a flushing settlement: the sweeps that book the trader side's SOL
+        // come before any deposit.
+        return Err(MathError::WipeSweepPending);
+    }
     if p.total_shares == 0 {
-        // R4A-7, and since AIR-08 also after the last lender's exit: SOL that reaches the
-        // trader while `S = 0` is the wiped lenders' (the live wipe record) or the protocol's
-        // (KI-10, R3A-10). A first deposit would make the sweep vest it to the new shares (and
-        // the last-lender flush pay it back at once), so the sweeps that book it come first.
-        let need = if rewards_required {
-            WIPE_TRADER_SWEPT | WIPE_REWARDS_CLAIMED
-        } else {
-            WIPE_TRADER_SWEPT
-        };
-        if p.wipe_swept & need != need {
-            return Err(MathError::WipeSweepPending);
-        }
         if p.is_wipe_live() {
             // V2-12: the full-wipe record is final from now on; a parked depositor converts.
             p.wipe_live = 0;
@@ -2171,12 +2169,16 @@
     let (mut p, mut v) = (*pool, *vault);
     let mut rec = wipe_record.as_deref().copied();
     v.lender_sol = add_u64(v.lender_sol, lamports)?;
-    if p.total_shares == 0 {
-        // R4A-7; AIR-08: with no live wipe record the SOL goes to the protocol (KI-10), and the
-        // bit lets the next deposit in.
-        p.wipe_swept |= source.bit();
+    let after_flush = p.total_shares != 0 && p.wipe_swept & source.bit() == 0;
+    // R4A-7; AIR-08: with no live wipe record the SOL goes to the protocol (KI-10), and the
+    // bit lets the next deposit in; after a flushing settlement it is the flushed shares'.
+    p.wipe_swept |= source.bit();
+    if after_flush {
+        release_in(&mut p, now)?;
+        credit_in(&mut p, &mut v, None, i128::from(lamports), true)?;
+    } else {
+        vest_in(&mut p, &mut v, rec.as_mut(), now, lump_vest_s, lamports)?;
     }
-    vest_in(&mut p, &mut v, rec.as_mut(), now, lump_vest_s, lamports)?;
     commit_record(wipe_record, rec);
     *pool = p;
     *vault = v;
@@ -2357,6 +2359,9 @@
         p.total_shares != 0 && ovf(p.token_claims()?.checked_mul(VEST_FLUSH_RATIO))? <= claims;
     if flushed {
         flush_vest_in(&mut p, now)?;
+        // R4A-7 after a flush: the trader side's SOL not yet swept is the flushed shares', so
+        // the next deposit waits for the sweeps, which credit it at once (`sweep`).
+        p.wipe_swept = 0;
     }
     let rec = maybe_roll_in(&mut p, &mut v, now)?;
--- a/programs/shortwin/src/instructions/sweep_trader.rs
+++ b/programs/shortwin/src/instructions/sweep_trader.rs
@@ -374,8 +374,8 @@
                 SweepSource::Trader,
             )
             .math()?;
-            if rewards_clear && pool.total_shares == 0 {
-                // AIR-08: after the last lender's exit a deposit waits for the sweeps too, the
+            if rewards_clear && pool.wipe_swept & pool::WIPE_REWARDS_CLAIMED == 0 {
+                // AIR-08 and a flushing settlement: a deposit waits for the sweeps too, the
                 // venue-reward bit on a pump venue included: set it as after a full wipe
                 // (M5-SEC-1), once the trader holds no claimable cashback.
                 pool::sweep(
```

---

[75] **6. apply_config installs a new governance key that never signed.**

`shortwin::apply_config` · Confidence: 75 · NEW

**Description**
apply_config writes a mistyped governance key from an honest Authorities change without its signature, so no signer can call propose_config again until a program upgrade.

**Fix**

```diff
--- a/programs/shortwin/src/instructions/apply_config.rs
+++ b/programs/shortwin/src/instructions/apply_config.rs
@@ pub struct ApplyConfig<'info> {
         bump = oracle_cache.bump
     )]
     pub oracle_cache: Option<Box<Account<'info, OracleCache>>>,
+    /// `Authorities` only: the new `governance`. It signs, so a key that nobody controls is refused.
+    pub new_governance: Option<Signer<'info>>,
+    /// `Authorities` only: the new `pause_authority`. It signs too.
+    pub new_pause_authority: Option<Signer<'info>>,
 }
@@ pub fn handler(ctx: Context<ApplyConfig>) -> Result<()> {
     match &change {
-        ConfigChange::Authorities { fee_recipient, .. } => {
+        ConfigChange::Authorities {
+            governance,
+            pause_authority,
+            fee_recipient,
+            ..
+        } => {
+            let new_governance = ctx
+                .accounts
+                .new_governance
+                .as_ref()
+                .ok_or_else(|| error!(ShortwinError::AccountMismatch))?;
+            require_keys_eq!(new_governance.key(), *governance, ShortwinError::Unauthorized);
+            let new_pause_authority = ctx
+                .accounts
+                .new_pause_authority
+                .as_ref()
+                .ok_or_else(|| error!(ShortwinError::AccountMismatch))?;
+            require_keys_eq!(
+                new_pause_authority.key(),
+                *pause_authority,
+                ShortwinError::Unauthorized
+            );
             let recipient = ctx
                 .accounts
                 .fee_recipient
```

---

[75] **7. Twelve-byte ELF fingerprint lets a venue hide an upgrade**

`shortwin::flag_venue_upgraded` · Confidence: 75 · NEW

**Description**
A venue upgrade authority finds a 96-bit fingerprint collision, so after one extension the latch does not trip and open_short accepts opens on unreviewed code.

**Fix**

```diff
--- a/programs/shortwin/src/state/venue_guard.rs
+++ b/programs/shortwin/src/state/venue_guard.rs
@@ -44,7 +44,8 @@
-/// Bytes of the trimmed ELF's sha256 the guard keeps (96 bits: a second preimage of a recorded
-/// fingerprint costs ~2^96 hashes).
-pub const GUARD_ELF_LEN: usize = 12;
+/// Bytes of the trimmed ELF's sha256 the guard keeps: all 32. The venue authority writes both
+/// ELFs, so a truncated prefix needs only a collision (2^(4*LEN) work), not a second preimage.
+pub const GUARD_ELF_LEN: usize = 32;
@@ -78,5 +79,5 @@
-    pub guard_elf: [[u8; 12]; 4],
+    pub guard_elf: [[u8; 32]; 4],
 // VenueGuard grows 224 -> 304 bytes: realloc it in a migration and update layout_asserts.rs.
```

---

[75] **8. A clock armed before a venue restart stays live after the warm-up.**

`shortwin-math::evaluate` · Confidence: 75 · NEW

**Description**
After a venue restart, any keeper settles an armed position as S1 or S2 at the first check, without 40 new EMA updates.

**Fix (Option A — validate)**:

```diff
--- a/crates/shortwin-math/src/settle.rs
+++ b/crates/shortwin-math/src/settle.rs
@@ pub struct SettleInputs {
+    /// `ema_count` at the market's last venue restart (0: none); set where `mark_restart` runs.
+    pub restart_count: u64,
@@ pub fn evaluate<Q: BoxQuotes>(
     if inp.executable {
-        let live = state.is_live(now, terms.arm_gap_s);
+        // A clock armed before the last venue restart re-arms on the new venue.
+        let live = state.is_live(now, terms.arm_gap_s) && state.count >= inp.restart_count;
```

**Fix (Option B — restrict)**:

```diff
--- a/crates/shortwin-math/src/settle.rs
+++ b/crates/shortwin-math/src/settle.rs
@@ pub struct SettleInputs {
     /// `ema_count` after this instruction's roll.
     pub ema_count: u64,
+    /// `ema_count` at the market's last observation restart (a pool adoption, `-> PumpSwap`);
+    /// the program stores it next to every `mark_restart` (Market._reserved).
+    pub restart_count: u64,
@@ pub fn evaluate<Q: BoxQuotes>(
     let expired = inp.expired();
+    // An arm from before the last restart counted updates on another venue and through the
+    // unchecked warm-up: it is lapsed (re-armed or disarmed below), never live.
+    if state.armed != 0 && state.count < inp.restart_count {
+        state.last = i64::MIN;
+    }
```

**Fix (Option C — ban-path)**:

```diff
--- a/crates/shortwin-math/src/settle.rs
+++ b/crates/shortwin-math/src/settle.rs
@@ -579,5 +579,9 @@ pub fn evaluate<Q: BoxQuotes>(
     let now = inp.now;
     if inp.executable && !inp.warmed {
         debug_assert!(!s3_due(&inp.s3, &terms.s3, now, true));
+        // A restart voids the clock's evidence (warm-up seeds, the old venue's updates):
+        // lapse it, so the first evaluation after the gate re-arms with a fresh count.
+        state.last = i64::MIN;
         return Ok(Evaluation::default());
     }
--- a/programs/shortwin/src/instructions/sol_settle.rs
+++ b/programs/shortwin/src/instructions/sol_settle.rs
@@ -265,3 +265,5 @@ fn evaluate<'info>(
     let ev = if graduated {
+        // The restart lapses the clock (as `settle::evaluate` does while warming).
+        arm.last = i64::MIN;
         Evaluation::default()
     } else {
```

---

[75] **9. The whole-bps floor of λ lets a deposit lower locked_depth.**

`shortwin-math::locked_depth` · Confidence: 75 · NEW

**Description**
An attacker who deposits into a low-λ pool cuts D by up to 1/λ_bps, so windows cancel, opens fail and D_inc falls; also in propose_pool.

**Fix**

```diff
--- a/crates/shortwin-math/src/pool_select.rs
+++ b/crates/shortwin-math/src/pool_select.rs
@@ -41,11 +41,15 @@ pub fn locked_depth(lp_counter: u64, lp_supply: u64, x: u64, y: u64) -> Result<u128, MathError> {
     let k = u128::from(x)
         .checked_mul(u128::from(y))
         .ok_or(MathError::Overflow)?;
-    mul_div(
-        u128::from(lambda_bps(lp_counter, lp_supply)),
-        sqrt_u128(k, Rounding::Floor),
-        u128::from(BPS),
-        Rounding::Floor,
-    )
+    // One floor on the raw ratio: (counter − supply)/counter is unchanged by a proportional
+    // deposit or withdrawal, so D is too (up to the venue's own LP rounding).
+    let locked = match lp_counter.checked_sub(lp_supply) {
+        Some(l) if lp_counter > 0 => l,
+        _ => return Ok(0),
+    };
+    mul_div(
+        u128::from(locked),
+        sqrt_u128(k, Rounding::Floor),
+        u128::from(lp_counter),
+        Rounding::Floor,
+    )
 }
```

---

[75] **10. A keeper raises the PumpSwap risk fee in its own liquidation transaction.**

`shortwin::read_state` · Confidence: 75 · NEW

**Description**
A keeper calls set_coin_creator on a default-creator pool before liquidate, so the risk fee rises and the keeper gets a healthy position's penalty share.

**Fix**

```diff
--- a/programs/shortwin/src/venues/pump_amm.rs
+++ b/programs/shortwin/src/venues/pump_amm.rs
@@ -701,9 +701,13 @@ pub fn read_state(
     let tiers = math_tiers(fc.fee_tiers);
+    // pAMM `set_coin_creator` takes no signer (U32): any caller can give a default-creator pool
+    // a coin creator, which adds the tier's creator fee at once. Price that fee from the start,
+    // so no caller can raise every risk quote inside one transaction.
     let schedule = supply.map(|s| PammFeeSchedule {
         tiers: &tiers,
         base_supply: s,
         creator_fee_configurable: gc.creator_fee_configurable(),
         pool_creator_fee_bps: pool.creator_fee_bps,
-        coin_creator_is_default: pool.coin_creator_is_default(),
+        coin_creator_is_default: false,
         cap_bps: None,
     });
```

---

[75] **11. Correctness: sol_settle tests S1 at a lower fee than liquidate's buyback**

`shortwin::sol_settle` · Confidence: 75 · NEW

**Description**
sol_settle tests S1 at P_ema's lower PumpSwap fee while liquidate refuses the buyback at the live fee, so no keeper can exit; also in expire.

**Fix**

```diff
--- a/programs/shortwin/src/instructions/sol_settle.rs
+++ b/programs/shortwin/src/instructions/sol_settle.rs
@@ -212,7 +212,10 @@
     let risk = venues::risk_quoter(
         &market,
         &state,
         rem.read,
-        state.exit_risk_cap_bps(u64::from(pos.venue_fee_hard_cap_bps)),
+        // The S1 budget legs read the fee that `liquidate`'s `buyback_allowed` reads
+        // (min(φ_live, hard cap)); P_ema's tier opens a band where neither exit runs.
+        // Trade-off: this gives back AIR-10's sale protection on sol_settle's quotes.
+        u64::from(pos.venue_fee_hard_cap_bps),
     )
     .ok_or(ShortwinError::NoSupportedVenue)?;
--- a/programs/shortwin/src/engine/bounds.rs
+++ b/programs/shortwin/src/engine/bounds.rs
@@ -110,6 +110,7 @@ fn reference(
     let l_live = state.liquidity().unwrap_or(obs.ema.liq);
     let l_b = bounds::ref_liquidity(l_live, &obs.ema, &obs.ring, depth_cap).math()?;
-    // AIR-10: the risk fee at most φ at `P_ema` on PumpSwap (`VenueState::exit_risk_cap_bps`),
-    // so a sale earlier in the exit's transaction cannot lift `B̂_ref` and `max_in` either.
-    let cap = state.exit_risk_cap_bps(u64::from(pos.venue_fee_hard_cap_bps));
+    // `B̂_ref` takes the fee of the `b_live` that `plan` checks against `max_in` (min(φ_live,
+    // hard cap)): at P_ema's lower tier the fee step uses up δ_exp, and `expire` fails.
+    // Trade-off: this gives back AIR-10's sale protection on `B̂_ref` and `max_in`.
+    let cap = u64::from(pos.venue_fee_hard_cap_bps);
     let b_ref = state.quoter().and_then(|pr| {
```

---

[75] **12. Any caller redirects the PumpSwap cashback to a WSOL account nobody sweeps**

`shortwin::sweep_trader` · Confidence: 75 · NEW

**Description**
Any caller claims the trader's PumpSwap cashback into a new WSOL account that sweep_trader never closes, so the lenders never receive it.

**Fix**

```diff
--- a/programs/shortwin/src/instructions/sweep_trader.rs
+++ b/programs/shortwin/src/instructions/sweep_trader.rs
@@ -236,8 +236,9 @@ pub fn handler<'info>(ctx: Context<'info, SweepTrader<'info>>) -> Result<()> {
     } else {
         (None, ctx.remaining_accounts)
     };
-    require_keys_eq!(
-        ctx.accounts.trader_wsol_ata.key(),
-        wsol_ata,
-        ShortwinError::AccountMismatch
-    );
+    // pAMM `claim_cashback` pays into any WSOL account the trader owns (R-VEN Update H7), so the
+    // sweep closes any of them, not only `ATA(trader, WSOL)`: `close_trader_ata` checks the token
+    // program, the owner (`trader`) and the mint (WSOL). NOTE: on PumpSwap the venue-reward bit
+    // must also wait until the pAMM UVA's `total_cashback_claimed` is covered by these closes.
+    let _ = wsol_ata;
```

---

[55] **13. Liquidation pays the keeper nothing when rent takes the whole remainder.**

`shortwin-math::liquidate` · Confidence: 55 · NEW

**Description**
The waterfall pays rent before the keeper floor, so a keeper earns 0 from a liquidation at V_eff ≤ B but earns K_s from S1.

---

Findings List

| # | Confidence | Title |
|---|---|---|
| 1 | [90] | A rebase gives a locked lender a share worth all free tokens |
| 2 | [90] | open_short does not check the age of P_ema. |
| 3 | [90] | The PumpSwap price start keeps P_ema at a pushed-down spot |
| 4 | [90] | A deposit after a halted-venue S3 takes the old lenders' vested SOL |
| 5 | [80] | After a dust flush a depositor takes the pre-wipe cashback |
| 6 | [75] | apply_config installs a new governance key that never signed. |
| 7 | [75] | Twelve-byte ELF fingerprint lets a venue hide an upgrade |
| 8 | [75] | A clock armed before a venue restart stays live after the warm-up. |
| 9 | [75] | The whole-bps floor of λ lets a deposit lower locked_depth. |
| 10 | [75] | A keeper raises the PumpSwap risk fee in its own liquidation transaction. |
| 11 | [75] | Correctness: sol_settle tests S1 at a lower fee than liquidate's buyback |
| 12 | [75] | Any caller redirects the PumpSwap cashback to a WSOL account nobody sweeps |
| | | **Below Confidence Threshold** |
| 13 | [55] | Liquidation pays the keeper nothing when rent takes the whole remainder. |

---

## Leads

_Vulnerability trails with concrete code smells where the full exploit path could not be completed in one analysis pass. These are not false positives — they are high-signal leads for manual review. Not scored._

- **The extend command never adds a market's keys to that market's table.** — `keeper::main` · NEW — Code smells: The extend arm (build_alt.rs:148-155) always builds entries(Some(c), a.static_only), the protocol list, and ignores --market. The market arm (build_alt.rs:190-225) only creates a new table. The alt::market_entries docs (alt.rs:296) say the keeper can add the keys of a new extras set or of a pool switch, but no code calls alt::extend with market_entries. extend --table <market table> adds the protocol addresses to the market table, and that rent comes back only when the table closes. The check command compares a table with the protocol list only. — The extend command adds only the protocol address list, so no command adds a pool switch's keys to a market's table. We did not find a transaction that then fails.
- **The v1 probe prints `landed` after a refused v1 transaction.** — `keeper::run` · NEW — Code smells: The summary note branches on a.send alone (v1_probe.rs:141), and the verdict word branches on v1_ok (v1_probe.rs:139). RpcChain::send returns Ok with err = Some(TxError::Rejected) on a refusal (services/common/src/rpc.rs:386-392), so accepted and v1_ok are false while the note says that the transaction landed. docs/audit/scope.txt lists this tool as [context]. — With --send, run prints `landed: leaders take v1` next to REFUSED when the v1 send fails, so the operator reads a contradictory verdict. The exit code 1 is correct, and we found no other effect.
- **A pool switch keeps the market's route-broken flag set.** — `shortwin::adopt` · NEW — Code smells: adopt (pool_select.rs:615-619) sets no_route_since, last_no_route_obs, pool_fail_since and last_pool_fail_obs to 0 and clears the window, but leaves MARKET_FLAG_ROUTE_BROKEN set. route::s3_no_route and route::position_no_route OR in route_broken (route.rs:35-37), so liquidatable or expired positions become S3-eligible after s3_wait_s on a pool that executes. guard::open_static keeps refusing RouteBroken (guard.rs:92-95). Only set_route_broken, which the Emergency or Governance signs with no timelock, clears the flag. pool_fail never reads the flag (V1-7), so a flagged market can switch pools and keep it. The docs do not say whether a switch clears the flag. — adopt clears both route chains but keeps MARKET_FLAG_ROUTE_BROKEN, so the S3 chain starts again at once on the new pool. This assumes that the flag describes the old pool's route, and we did not prove a loss.
- **check_fee_recipient accepts System-owned PDAs of short.win as the recipient.** — `shortwin::apply_config` · NEW — Code smells: check_fee_recipient (config.rs:818-826) tests only owner == System program and !executable. The event authority PDA ['__event_authority'], every ['trader', market] PDA, the ['trader_wsol', market] address between legs and ['swap_auth', position] are System-owned and empty, so they pass. No instruction signs a System transfer out of the event authority, so fees sent there stay there. engine::trader::normalise_pdas and sweep_trader move trader lamports to that market's market_vault and credit them to its lenders. The doc comment says the check refuses every short.win account. initialize_config takes the recipient by key with no check. sweep_protocol_fees needs no signer, so any caller can move every market's fees before another Authorities change (about 96 h). — check_fee_recipient accepts short.win's own System-owned PDAs, so after a mistaken Authorities change any caller sweeps protocol fees to the event authority or a trader PDA. Only a Governance key error installs such a recipient.
- **check_shape accepts any feed id in an OracleFeed change.** — `shortwin::apply_config` · NEW — Code smells: ConfigChange::check_shape for OracleFeed checks only account != Pubkey::default() (config.rs:749-752) and never reads feed_id. initialize_config writes c.pyth_feed_id = args.pyth_feed_id with no check (initialize_config.rs:145). Program logic never reads PYTH_SOL_USD_FEED_ID (constants.rs:311); only services/ops does. crank_sol_usd binds only to the configured pair, so a receiver-owned PriceUpdateV2 of another USD feed and its feed id pass every rule, and the step rule is skipped on the cache that apply_config empties. tier::assign then values liquidity at that price against theta_a_usd_e6 and theta_b_usd_e6, so a pool can tier A instead of C. Positions keep the terms they locked at open. — check_shape accepts any OracleFeed feed id, so after a mistaken Governance change crank_sol_usd caches another asset's price as SOL/USD and new opens get looser tiers. We did not prove that such a change passes the off-chain config gate.
- **apply_config installs an OracleFeed account it never reads.** — `shortwin::apply_config` · NEW — Code smells: ConfigChange::check_shape for OracleFeed requires only account != Pubkey::default() (config.rs:749-752). The OracleFeed arm of apply_config (apply_config.rs:98-108) takes no PriceUpdateV2 and checks neither its owner nor its feed id, but the Authorities arm takes the new fee_recipient and runs check_fee_recipient (apply_config.rs:85-96). CrankSolUsd requires address = config.pyth_sol_usd and owner = PYTH_RECEIVER_ID, and pyth::check compares the feed id, so a wrong key or feed id makes every crank fail. The same apply empties OracleCache, so tiers read a never-cranked cache (ARCH §4.7). Recovery needs another OracleFeed change: timelock_s plus the Squads time lock, about 96 h (KI-45). Only an honest Governance error reaches this. — apply_config installs an OracleFeed account with no owner, layout or feed-id check and empties the cache, so a mistyped key makes every crank_sol_usd fail. We did not prove a loss, because tiers fall to C, the tier with the largest cushion, until another timelocked change.
- **Any caller can fund the boost vault ATA and block PumpSwap opens** — `shortwin::boost_blocks` · NEW — Code smells: boost_blocks (pump_amm.rs:781-784) refuses opens while boost_amount > boost_dust_lamports and now < graduated_at + boost_window_s. boost_amount is the WSOL amount of ATA(boost_vault, WSOL), and the reader checks only the token program, the mint and the token owner. Any payer can create that ATA and send WSOL to it without the owner's signature. The defaults are boost_dust_lamports = 50,000,000 and boost_window_s = 900. Exits never read the gate. No doc comment or known-issues row states the third-party trigger. — An attacker who sends just over 0.05 SOL of WSOL to the boost vault ATA makes open_short fail until graduated_at + 900 s. We assume the gate is meant to react only to a boost that pump funds.
- **The execution fit reads the candidate's live real SOL and spot price.** — `shortwin::candidate_tier` · NEW — Code smells: candidate_tier passes r_now: a.state.r (:446), the live real SOL of the candidate in the same transaction. carried_caps drops only X_cand at FitAt::Execution (x_now = u64::MAX, :490-493). carried_fit judges first at window_price = min(P_ema, live P_cand) (:532). The docs say that the execution reads only terms that no transaction can move (:86-88; execute_pool_switch.rs:38-41). An unlocked LP's withdrawal also lowers r_now. The tier feeds only impact_pos_bps and impact_mkt_bps, and DEFAULTS sets them to 500 and 2,000 for tiers A, B and C. TierImpactPosBps (0..1,000) and TierImpactMktBps (0..5,000) let governance set them apart. — An attacker who sells into the candidate before execute_pool_switch lowers r_now and P_cand, so the tier drops and tighter tier caps cancel the window. The default tier table gives tiers A, B and C equal impact caps, so this needs a governance change first.
- **The fee-scaled caps count the creator fee as a manipulation cost.** — `shortwin::cap_fee_ppm` · NEW — Code smells: raydium_cpmm::state_of sets fee_ppm to PoolState::fee_ppm = trade_fee_rate + creator_fee_rate (venues/raydium_cpmm.rs:227; shortwin-venue-state raydium_cpmm.rs:423). VenueState::cap_fee_ppm (venues/mod.rs:796-806) passes it on unchanged. open_short gives it as phi_ppm (open_short.rs:383) to fee_size_cap (3·φ·X₀) and fee_oi_cap (7.5·φ·X₁). On PumpSwap φ is lp + protocol + creator. ECON line 969 says that a push costs about φ times the liquidity moved. ECON §12.8 says that the coin creator earns the creator fee on every leg, and CPMM collect_creator_fee pays pool_creator. No source line accepts the rebate. market_max_notional (25 SOL) bounds the basket. — A CPMM pool creator or PumpSwap coin creator gets the creator fee back, so its price push costs less than the φ the caps use. We did not simulate the forced liquidations on a creator-fee pool, or check who collects LaunchLab creator fees.
- **carried_caps takes the execution tier from the candidate's live SOL reserve** — `shortwin::carried_caps` · NEW — Code smells: `carried_caps` removes the live X_cand at `FitAt::Execution` (pool_select.rs:490-493) but calls `candidate_tier(market, a, ctx, l_slow, p)` without `at` (:496). `candidate_tier` passes `r_now: a.state.r` (:446), the live WSOL reserve of the candidate, to `tier::liquidity_lamports`, where R_eff = min(R_now, R_slow, locked/λ_min). The first evaluation runs at p = min(P_ema, P_cand) (`window_price`, :532), and a dump lowers the live P_cand. A `crank_sol_usd` in the same transaction also refreshes the cached oracle that the tier reads. The tier sets `impact_pos_bps` and `impact_mkt_bps` of `CarriedCaps`; a failed fit drops the adoption (execute_pool_switch.rs:185-189), and `clear_window` cancels the window. The `FitAt::Execution` doc, the module docs and the test `a_withdrawal_wrapped_around_the_execution_does_not_cancel_the_window` (tests/litesvm/tests/pool_select_it/carried_fit.rs:344-351) say that a wrapped withdrawal or buy cannot cancel an honest window; the test uses the default tiers only. The per-tier caps have bounds [0, 1,000] and [0, 5,000] bps and no ordering rule (params.rs:1064-1065). — At FitAt::Execution, carried_caps passes the live a.state.r to candidate_tier, so a seller or unlocked LP around execute_pool_switch lowers the tier and can cancel the window. We did not prove a cancel at the default tiers, where A, B and C share impact caps and the fee-scaled caps bind first.
- **check_change validates the stored parameters for a non-parameter change.** — `shortwin::check_change` · NEW — Code smells: For `Authorities`, `Timelock`, `EnableVenues` and `OracleFeed`, `GlobalConfig::param_set_after` returns `self.param_set()`, the stored set (config.rs:149-152). `check_change` runs `validate_param_set` on that set (config.rs:161), and `propose_config` (propose_config.rs:34) and `apply_config` (apply_config.rs:78) both call it. An upgrade can tighten a compiled bound with no `CONFIG_VERSION` bump, as AIR-06 did when it raised `MIN_NOTIONAL_FLOOR` to 53,000,000. After such an upgrade, every non-parameter proposal fails with `InvalidParams`, and a pending `Authorities` change lapses after `CONFIG_APPLY_WINDOW_S`. Governance must then apply a `Params` change first, one more timelock, before it can rotate a key. — check_change also validates the stored parameters, so after an upgrade that tightens a compiled bound, Governance cannot propose or apply an Authorities change. We did not prove that a deployed config holds a value that a later bound refuses.
- **check_change revalidates stored parameters for a key or oracle change** — `shortwin::check_change` · NEW — Code smells: param_set_after returns the live self.param_set() for Authorities, Timelock, EnableVenues and OracleFeed (config.rs:149-152); check_change runs validate_param_set on it at propose_config and apply_config; cancel_config lets the Emergency cancel a Params change because GOVERNANCE_ONLY_CANCEL holds only KIND_AUTHORITIES and KIND_TIMELOCK; the compiled floor MIN_NOTIONAL_FLOOR was already raised once by AIR-06; no instruction migrates GlobalConfig after an upgrade. — check_change validates the live parameter set for an Authorities, Timelock, EnableVenues or OracleFeed change that does not touch the parameters. If a later program upgrade tightens a compiled bound below a stored value, Governance cannot apply such a change, which no upgrade does today.
- **check_fee_recipient accepts a System-owned short.win PDA** — `shortwin::check_fee_recipient` · NEW — Code smells: check_fee_recipient tests only owner == system_program::ID and !executable; a market's trader PDA is System-owned with zero data (state/mod.rs), so it passes both checks; sweep_protocol_fees pays protocol_fees to the recipient and normalise_pdas then moves them to that market's market_vault; the doc comment claims the check refuses every short.win account; the path needs a Governance-approved Authorities change that names the PDA. — check_fee_recipient checks only the System owner and the executable flag, so it accepts a market's trader PDA, which is System-owned with zero data. Its doc comment says it refuses every short.win account, so an Authorities change that names such a PDA sends protocol fees to that market's lenders.
- **check_trade skips key checks when a venue account does not parse** — `shortwin::check_trade` · NEW — Code smells: pump_curve.rs:368-377 checks T7 only inside `if let Some(global) .. if let Ok(data) .. if let Ok(g) = pump::read_global(..)`. pump_amm::check_trade checks T9 and T16 only when read_global_config parses (pump_amm.rs:523) and T11 and T12 only when parse_pool parses (pump_amm.rs:548); pump_amm::check_read checks P6 the same way (pump_amm.rs:407). pump_curve.rs:347-350 says an unparsed Global leaves the venue not executable, so no leg runs. — pump_curve::check_trade and pump_amm::check_trade skip the T7, T9, T11, T12 and T16 key checks when Global, GlobalConfig or the pool does not parse. Today the executable check stops the leg; a future leg without it passes unchecked writable accounts to the venue CPI.
- **claim_sol runs the lender touch at the raw clock, not market_now** — `shortwin::claim_sol` · NEW — Code smells: `let now = clock.unix_timestamp;` at claim_sol.rs:76 (also cover_deficit.rs:61 and sync_lender.rs:104) goes to pool::touch and pool::claim. prologue::market_now (prologue.rs:50-57) clamps the clock for deposit, withdraw and sweep_trader, because its doc says that the clock is not guaranteed to be monotonic. Today the touch reads the time only in cal_advance, which returns early for an earlier day, and in the drop of an expired lock, so nothing fails. — claim_sol passes clock.unix_timestamp to pool::touch and pool::claim, and deposit and withdraw use market_now. A future check against vest_last or last_accrual_ts in claim_sol fails after a clock step back, which Agave's update_clock prevents today.
- **claim_sol moves the lock calendar past the market's last accrual time** — `shortwin::claim_sol` · NEW — Code smells: claim_sol.rs:76 passes the raw clock to pool::touch, and its cal_advance (pool.rs:761-787) drops the day buckets and moves cal_day. claim_sol does not write Market.last_accrual_ts, so a later withdraw at market_now can run behind cal_day. lock_of (pool.rs:1715-1720) decides from now alone that a lock is live. withdraw_in then calls cal_add on a dropped day (pool.rs:1843-1849) and gets InvalidInput (pool.rs:833). prologue.rs:50-54 says that market_now keeps withdrawals live across a clock step back. — claim_sol advances MarketVault.cal_day at the raw clock and lock_of ignores cal_day, so after a clock step back a lock holder's withdraw fails. Agave's update_clock prevents the step back today.
- **classify_pump_with reads a pump-owned curve with another discriminator as NotPump** — `shortwin::classify_pump_with` · NEW — Code smells: detect.rs:198-199 returns `Ok(PumpClass::NotPump)` for a pump-owned account at PDA(pump, ['bonding-curve', mint]) whose first 8 bytes are not `BONDING_CURVE_DISCRIMINATOR`; only pump can create an account at that PDA, so the account is the curve of this mint in another layout; the same function refuses a tagged curve that does not parse (`PumpVenuePrecedence`, detect.rs:201-202), and `canonical_pool_read` reads a pAMM account that does not parse as `CanonicalPool::Unreadable` (M5-SEC-5), so only this branch fails open; the module doc (detect.rs:45-48) says a Raydium adoption needs the proof that no pump venue applies; `create_market` with a Raydium hint (`require!(!class.pump_venue())`, create_market.rs:538-539) and `propose_pool` / `execute_pool_switch` on a list-only market (`incumbent()`, propose_pool.rs:182-192) take `NotPump` as that proof; the Raydium listing latches `GUARD_MASK_RAYDIUM` only (create_market.rs:547) and the proposals latch the bit of the candidate only, so a flagged pump upgrade does not stop them; the test `classifies_curves` pins `NotPump` for a flipped tag byte (detect.rs:283-288). — After a pump upgrade that changes the curve discriminator, any caller lists a live pump coin on Raydium, because classify_pump_with returns NotPump. No current pump build re-tags a curve.
- **classify_pump_with returns NotPump without a read of the canonical pool** — `shortwin::classify_pump_with` · NEW — Code smells: detect.rs:194-195 returns `Ok(PumpClass::NotPump)` when `*bc.owner != PUMP_PROGRAM_ID`, before the only `canonical_pool(cp, mint, authority)` read at detect.rs:214; the canonical-pool address depends only on the mint and `pool_authority(mint)`, and every caller key-checks it; only the pool-authority PDA of pump can create that pool, so a valid or unreadable pool there is a pump venue whatever the curve address holds; `create_market` (a Raydium hint passes `require!(!class.pump_venue())`, and `Auto` lists the coin as `None`), `propose_pool` and `execute_pool_switch` on a list-only market (`pump_venue = class.pump_venue()`, propose_pool.rs:192) and `crank_price::redetect` (crank_price.rs:201 returns `Ok(None)` for `NotPump`, so the market never moves to PumpSwap) all trust this result; the Raydium paths latch only the Raydium bits, not `GUARD_PUMP` or `GUARD_PUMP_AMM`. — If pump closes a migrated curve account, any caller lists the coin on Raydium, although a valid canonical PumpSwap pool exists. We found no pump instruction that closes a curve today.
- **close_short reports every trader sweep as SWEEP_TO_VEST.** — `shortwin::close_short` · NEW — Code smells: close_short.rs:255-256 writes to: SWEEP_TO_VEST and wipe_bits: 0 after pool::sweep(.., None, .., SweepSource::Trader). With total_shares == 0, pool::sweep sets wipe_swept, and credit_in moves the lamports from lender_sol to protocol_fees (shortwin-math pool.rs:2174-2179, 1039-1047). sweep_trader computes to from total_shares and wipe_live (sweep_trader.rs:313-320) and reports the wipe_swept bits. The wipe_swept bits stay set after a deposit, so wipe_bits: 0 can differ from them today. liquidate.rs:206, expire.rs:167 and open_short.rs:483 hardcode the same values. The indexer adds the lamports to its lender_sol mirror when to = 0 (services/indexer/src/ingest/project.rs:1290-1310). — close_short emits TraderSwept with to = SWEEP_TO_VEST and wipe_bits = 0, but pool::sweep sends the lamports to protocol_fees when total_shares is 0. If withdraw's LastSharesBorrowed check changes, the indexer books protocol SOL as lender SOL.
- **close_short accepts a position in any status.** — `shortwin::close_short` · NEW — Code smells: Only add_margin requires position.status == POSITION_OPEN (add_margin.rs:65). The position account of CloseShort checks only has_one = borrower, has_one = market and is_pda (close_short.rs:84-90). Liquidate (liquidate.rs:122-128), Expire (expire.rs:83-89) and SolSettle (sol_settle.rs:122-127) do the same. POSITION_OPEN is 0, the value of zeroed memory, and POSITION_EXIT_IN_PROGRESS = 1 is reserved for v1.1 (state/position.rs:18-21). open_short.rs:592 is the only write of status. — close_short never reads position.status, so after a v1.1 change that writes ExitInProgress, the borrower can close a position while another exit runs. No instruction writes that status today.
- **crank_price re-detection seeds P_ema with no open warm-up.** — `shortwin::crank_price` · NEW — Code smells: redetect writes Observation::new(now, p, l) at this transaction's curve spot for an unpriced market (crank_price.rs:216-222). It calls no mark_restart and leaves created_at at the listing time. The PumpCurve branch of open_short checks only GraduationHalted (open_short.rs:319-334); ema_warm is checked only on Raydium (300) and PumpSwap (317). market_warmup_s counts from created_at (open_short.rs:338-342). A curve listing sets created_at = now and gets the 50-s wait; the re-detection gets none. So spot_near_ema compares the spot with an EMA from that same spot, and curve_reserves returns the live vT and rT. tests/litesvm/tests/pump_it/graduation.rs:432 reaches the path only by deleting the curve account at listing, and it warps market_warmup_s before the open. — crank_price seeds P_ema of a re-detected curve market from a spot the caller can move, and open_short accepts opens on it at once. We found no current way for a list-only market to hold a live curve.
- **crank_sol_usd refuses large moves while tiers still use the old price.** — `shortwin::crank_sol_usd` · NEW — Code smells: pyth::check skips step_ok only when now - cached.publish_time > stale_s (pyth.rs:353), and crank_sol_usd passes stale_s = p.oracle_stale_s (crank_sol_usd.rs:86). tier::assign treats the same cache as fresh while now - publish_time <= oracle_stale_s (tier.rs:362). The compiled bounds accept oracle_max_step_bps = 500 and oracle_stale_s = 86,400 (params.rs:1053-1055), so a 5 % move between cranks can hold the tier price for 24 h. The params.rs doc says oracle_max_age_s limits the cache age for tiering, but tier::assign never reads it. ARCH §2.3.3 and OWNER-M7-15 state that the step rule binds only against a cache at most oracle_stale_s old, and they discuss a stalled feed, not a live feed that the step rule refuses. — After SOL/USD moves more than oracle_max_step_bps between accepted cranks, crank_sol_usd refuses each update until the cache is oracle_stale_s old, and tiers keep the old price. We did not show a SOL move above the step limit between two accepted cranks.
- **The Raydium listing ignores the pump latch bits** — `shortwin::create_market` · NEW — Code smells: On a Raydium hint, `create_market` runs `detect::classify_pump_with(bc, cp, ..)` with the compiled pump and PumpSwap readers (create_market.rs:538), then `guard::latch(.., VenueKind::RaydiumV4)` (:547). That mask is `GUARD_MASK_RAYDIUM`, bits 3 and 4 only (constants.rs:146; state/market.rs:96), so nothing reads the pump, PumpSwap and pump-fees bits (0, 1, 2). The comment at :540-546 keeps the latch because the listing fixes the pool permanently. A Raydium market never runs `classify_pump` again: `incumbent()` classifies only a `VenueKind::None` market (propose_pool.rs:160-200). `propose_pool` and `execute_pool_switch` on a list-only market also classify with the compiled pump readers and latch only the Raydium program of the candidate (propose_pool.rs:235; execute_pool_switch.rs:170). — create_market decides D1 with the compiled pump readers on a Raydium hint, but it checks only the Raydium latch bits. We did not prove that a pump upgrade makes those readers see a live pump coin as NotPump; that misread fixes the Raydium listing permanently.
- **Latch readers load VenueGuard without its version check** — `shortwin::create_market` · NEW — Code smells: `CreateMarket` (create_market.rs:200), `ProposePool` (propose_pool.rs:114), `ExecutePoolSwitch` (execute_pool_switch.rs:92) and `OpenShort` (open_short.rs:121) bind `venue_guard` by `seeds = [VenueGuard::SEED]` and the stored bump only. `FlagVenueUpgraded` (flag_venue_upgraded.rs:54) and `RevalidateVenue` (revalidate_venue.rs:51) also require `venue_guard.load()?.version == VENUE_GUARD_VERSION`. `initialize_config` writes `g.version = VENUE_GUARD_VERSION` (initialize_config.rs:157). Every reader of `Market` and `GlobalConfig` checks its version. `VENUE_GUARD_VERSION` is 1 and nothing migrates the account, so no path exists today. — create_market, propose_pool, execute_pool_switch and open_short read the VenueGuard latch bits without the version check that its writers make. A later layout change that skips a migration makes them read the wrong bits, so a pending upgrade no longer blocks listings, adoptions or opens.
- **The first create_market caller picks the venue, pool and market age** — `shortwin::create_market` · NEW — Code smells: The market account is `init` on `[Market::SEED, mint]`, so the first listing is the only one. `venue_hint` and the `hint_accounts` ReadSet are caller inputs (create_market.rs:506-570). `Adoption::apply` (create_market.rs:664) writes the venue, the pool cache, the SOL side flag, `pool_d`, `graduated_at` (CPMM: min(now, open_time) when open_time >= T_CLAMP; AMM v4: now) and `pool_seen_at`. `pool_select::adopt` writes `graduated_at` again only for a market that comes from `VenueKind::None` (pool_select.rs:600-602), so later Raydium adoptions keep the age of the first pool, and the tier reads that age (C below 24 h, A from 7 d). The module doc at create_market.rs:99-100 says that the mint and `config` fix every field but `creator`. Inside the listing window, where opens wait, a pool 5 % deeper replaces a thinner first pool. — The first create_market caller picks venue_hint and the Raydium pool, and later Raydium adoptions keep that pool's graduated_at. We did not prove a loss; the module doc says only creator depends on the caller.
- **curve_pricing takes the PumpSwap fee tier at a fixed 10¹⁵ supply** — `shortwin::curve_pricing` · NEW — Code smells: CurvePricingInputs::curve_pricing sets base_supply to 10¹⁵ (mod.rs:614) in the PammFeeSchedule of the post-migration pool. pricing_inputs receives the live mint supply but uses it only for the synthetic fee. pump_amm::read_state and PumpSwap take the tier on the live supply, and the PammFeeSchedule doc and the shortwin-math README name the live supply as the basis and assume 10¹⁵ before graduation. migrating_fv (FV) and curve_straddle (B̂_str on the EMA leg and the box) take this tier. A synthetic pool part can put the market cap just above the 420 SOL threshold; there a burn of 0.7 % moves the real tier from 120 to 125 bps, or from 25 to 95 bps with a default creator. — FV on a stuck migration takes the PumpSwap tier at a 10¹⁵ supply, so after a burn the borrower keeps one tier step of FV. We found no stuck migration without a PumpSwap halt of create_pool, because any signer can call migrate_v2.
- **Risk quotes past rT take a fixed seed of 206.9e12 tokens** — `shortwin::curve_pricing` · NEW — Code smells: CurvePricingInputs::curve_pricing returns pool_tokens = STANDARD_POOL_TOKENS (206.9e12) for every live curve and for a complete curve without a pool part (mod.rs:602). Only Quoter::buy_live replaces it with live_pool_tokens = curve_base_ata - rT. buy_xy (the EMA leg, D4), buy_repriced (the box, B̂_ref, the spot leg) and FV price the pool part on the constant. Both Global fixtures give token_total_supply - initial_real_token_reserves = 206.9e12, and the pump admin can change these values with set_params. — Risk quotes past rT take a fixed seed of 206.9e12 tokens, so a curve with a smaller seed gets quotes below the real buyback cost. Every SOL curve seeds 206.9e12 today, so only a pump set_params change opens the path.
- **deposit checks the absolute vault balance, not the received amount** — `shortwin::deposit` · NEW — Code smells: deposit.rs:179-184 runs require_gte!(pool_vault.amount, t_after) after transfer_checked and measures no delta. The buy legs measure the vault delta (raydium.rs:352-359). pool_vault often holds tokens above T: close_short books a buyback surplus as a donation (close_short.rs:279), sweep_base_ata of sweep_trader donates the tokens of the trader base ATA, and anyone can transfer tokens to it. The doc at deposit.rs:24-25 says that a fee mint fails at this check, which is false when the surplus covers the fee. Only check_mint_safety and its ALLOWED_EXTENSIONS (mint_filter.rs:49-55) keep fee mints out. — deposit compares pool_vault.amount with T, so donated tokens let a depositor get shares for tokens that the vault did not receive. This needs a transfer-fee mint, and the mint filter rejects every such mint today.
- **Pool selection adopts a pool on a venue the Emergency flagged** — `shortwin::execute_pool_switch` · NEW — Code smells: `propose_pool` (propose_pool.rs:235) and `execute_pool_switch` (execute_pool_switch.rs:170-171) refuse a candidate only through `guard::candidate_latch` / `candidate_latched`. Only `guard::open_static` (guard.rs:93-103) and `route::route_broken` (route.rs:35-38) read `config.venue_enabled` and `config.venue_route_broken`. `pool_select::adopt` sets `no_route_since`, `last_no_route_obs`, `pool_fail_since` and `last_pool_fail_obs` to 0 (pool_select.rs:615-618). On a flagged venue the S3 chain then starts again at the next observation, so S3 opens up to about 181 s later, plus the warm-up. The keeper pool monitor reads neither flag, so the keeper itself proposes such a pool. ARCH lines 419, 997 and 1005 and route.rs:34 give the effects of the flags as opens and S3 evidence only. — Any caller can move a market's open positions onto a venue that the Emergency disabled or flagged route-broken, and the adoption restarts the S3 timer. This assumes the flags must also bind pool selection, though ARCH lists only opens and S3 evidence; we proved no loss.
- **A 60-second observation gap cancels an armed collection window** — `shortwin::execute_pool_switch` · NEW — Code smells: `execute_pool_switch` runs `prologue::run` (execute_pool_switch.rs:150) before `window_adoptable` (:171). There, `settle::chain_step` sets `pool_fail_since = now` when `now - last_pool_fail_obs > S3_OBS_GAP_S` (60 s; crates/shortwin-math/src/settle.rs:747). `window_chain_holds` then needs `pool_fail_since + pool_fast_route_s <= fp_t0` (pool_select.rs:252-259), which fails, and `clear_window` cancels the window. The compiled bound `PoolFastWindowS` is (10, 120) (crates/shortwin-math/src/params.rs:1038), above the 60-s gap, so a window longer than 60 s adopts only after an observation of the incumbent inside it. At the default 60 s, an execution one second after the window end with no observation in between cancels. `propose_pool`'s ImproveWindow uses the same gate (`PoolWindowClosed`). The keeper cranks every 30 s when idle and keeps the `pool_fail` chain alive itself, so the gap needs a keeper outage. — The prologue of execute_pool_switch restarts pool_fail_since after a 60-s gap in observations of the incumbent, so the instruction cancels a valid window. The bound on pool_fast_window_s allows 120 s, and we did not prove such a gap while the keeper runs.
- **exit_risk_cap_bps caps the risk fee at P_ema on PumpSwap only** — `shortwin::exit_risk_cap_bps` · NEW — Code smells: exit_risk_cap_bps (mod.rs:822) returns the hard cap for every venue kind except PumpSwap. read_state_boxed_for_exit (mod.rs:1085) sets ema_fee only for PumpSwap, so fee_ppm_at_ema of the curve stays u32::MAX on liquidate, expire and sol_settle. The curve fee comes from the live market cap ⌊vQ·10¹⁵/vT⌋ (pump_curve.rs:454), and a sale in the same transaction lowers it. liquidate::assess (liquidate.rs:338), bounds::reference (bounds.rs:114) and sol_settle (sol_settle.rs:216) price the spot leg, the EMA leg on the cached ring, B̂_ref, the box and FV at that fee. Opens cap the curve fee at P_ema (cap_fee_ppm, M7-SEC-5), and AIR-10 caps only PumpSwap exits. The doc comment excludes the curve because of 'the fixed 10¹⁵ supply', which stops a burn but not a sale; KI-72 and KI-77 disclose only burns. The unit test exit_risk_cap_takes_the_fee_at_p_ema_on_pumpswap_only pins the exclusion. The pump-fees admin edits the FeeConfig with no program upgrade, so the venue latch does not react. KI-77 declines to cap φ_s at P_ema, so a fix can cap only CurvePricing::fees. — A keeper who sells on the curve before liquidate raises the fee tier of every exit risk quote and liquidates a healthy position. This needs a curve FeeConfig with more than one tier, and pump uses one flat tier today.
- **fee_view skips the consistent() check of read_state** — `shortwin::fee_view` · NEW — Code smells: `read_state` runs `consistent()` before it uses a pool: CPMM checks the WSOL side, the token mint, both token programs and the observation key (raydium_cpmm.rs:277), and AMM v4 checks the nonce, the WSOL side, the token mint and Tokenkeg (raydium_v4.rs:199); `risk_quoter` (mod.rs:1142-1143) calls `raydium_cpmm::fee_view` -> `live_fees` (raydium_cpmm.rs:321), which checks only `pool.amm_config` against the config key, and `raydium_v4::fee_view` (raydium_v4.rs:225), which parses only the pool slot; `live_fees` takes the creator-fee direction from the `sol_first` flag of the market, not from `pool.sol_is_token0()`; `sol_settle` uses this view when the pool cannot execute; the pool slot is key-checked against the pool cache of the market, and the fee is capped at `cap_bps`. — After a Raydium upgrade that makes the pool's mints or token programs mutable, fee_view prices risk on a pool that no longer matches the market. Today those fields are immutable, and the fee is capped at cap_bps.
- **Venue latch fails open after a loader-v4 migration** — `shortwin::flag_venue_upgraded` · NEW — Code smells: FlagVenueUpgraded.program_data and RevalidateVenue.program_data both pin owner == bpf_loader_upgradeable::ID and the compile-time loader-v3 venue_program_data PDA; read_deploy_slot maps every read error to VenueAccountMismatch; no path reads a loader-v4 program header; the loader-v3 Migrate instruction empties that ProgramData, so the latch fails open; loader-v4 migration is not verified active on mainnet. — flag_venue_upgraded requires the ProgramData owner to be the upgradeable loader. A venue that migrates to loader-v4 closes that account, so the owner constraint fails for every caller and the latch never trips.
- **graduate stores the PumpSwap vault keys and never reads the latch** — `shortwin::graduate` · NEW — Code smells: graduate and write_cache (pump_amm.rs:358-366, 820) store pool_base_token_account and pool_quote_token_account (@139, @171) from the compiled read_pool and read no VenueGuard. canonical_pool_read checks only the index, creator, base_mint, quote_mint and the mayhem flag. pump_curve::sync, crank_price's redetect_pool and create_market's init_pamm_market reach graduate; CrankPrice and SolSettle take no venue_guard account, and Auto listings are never latched, although a PumpSwap listing now fixes venue keys. No instruction rewrites the PumpSwap vault cache after graduation, and check_read forces P1 and P2 to the cached keys, so a wrong key leaves the market not executable with only S3. Raydium listings and adoptions call guard::latch for the same reader risk (create_market.rs:547, propose_pool.rs:235, execute_pool_switch.rs:170). — Any caller of crank_price or sol_settle makes graduate store the pool vault keys while a PumpSwap upgrade awaits review, and nothing rewrites a misread key. We did not prove that a pAMM upgrade moves the vault fields and keeps the canonical fields valid.
- **pool_vault has no token::mint or token::authority constraint in the exits** — `shortwin::liquidate` · NEW — Code smells: pool_vault in Liquidate (liquidate.rs:140-145), Expire (expire.rs:101-106) and CloseShort (close_short.rs:102-107) is Box<InterfaceAccount<TokenAccount>> with seeds [POOL_VAULT_SEED, market] and the stored bump only. It has no token::mint, token::authority or token::token_program constraint. Only create_market initialises the PDA, with initialize_account3(mint, authority = market). The post-condition pv.amount >= t_after trusts this account. — The exits trust the mint and authority of pool_vault only because create_market made that PDA. We found no path today that puts another account at that address.
- **liquidate, expire, sol_settle and close_short never read position.status** — `shortwin::liquidate` · NEW — Code smells: add_margin.rs:65 requires position.status == POSITION_OPEN. The position constraints of Liquidate (liquidate.rs:122-129), Expire (expire.rs:83-90), SolSettle (sol_settle.rs:122-128) and CloseShort (close_short.rs:84-91) check only has_one and is_pda. Only open_short writes status (open_short.rs:592), always POSITION_OPEN = 0, the value of zeroed memory. position.rs:21 reserves POSITION_EXIT_IN_PROGRESS = 1 for v1.1, and state/mod.rs:54 already defines SWAP_AUTH_SEED for the v1.1 exit. — Once a v1.1 exit writes POSITION_EXIT_IN_PROGRESS, a keeper or the borrower can still start a second exit on that position. No instruction writes that status today.
- **ShortPosition carries no layout version that the exits check** — `shortwin::liquidate` · NEW — Code smells: ShortPosition (position.rs:26) has no version field. Market and GlobalConfig carry version, and every exit checks MARKET_VERSION and CONFIG_VERSION. liquidate, expire, sol_settle, close_short and add_margin load Account<ShortPosition> by its discriminator alone. _reserved: [u8; 32] is kept for future locked terms (the O1/O2 note). — A v1.1 upgrade that adds locked terms in _reserved reads them as 0 on every older position, and the exits act on those zeros. Only one layout exists today, so no path exists now.
- **lp_supply accepts a 165-byte token account as an LP mint** — `shortwin::lp_supply` · NEW — Code smells: `raydium::lp_supply` (raydium.rs:94-102) checks only `*info.owner == TOKEN_PROGRAM_ID`, then calls `mint::read_base`, which refuses only `data.len() < MINT_LEN` (mint.rs:169-177); `mint::check` refuses an SPL Token mint that is not exactly 82 bytes (mint.rs:246), but `read_base` does not; on a 165-byte token account, the `COption` tag @0 is mint-key bytes 0..4, `supply` @36 is owner bytes 4..12, `is_initialized` @45 is owner byte 13 and the freeze tag @46 is owner bytes 14..18, and `InitializeAccount3` takes any owner key; the supply sets `lambda_bps`, so it drives `locked_depth`, `pool_d`, the eligibility floor and the size caps; `raydium_v4::parse` and `raydium_cpmm::parse` key-check the slot against `info.lp_mint` / `pool.lp_mint`, which Raydium writes, and `check_read` binds it to the cached `RAY_LP_MINT`. — lp_supply reads a 165-byte token account as an LP mint, so the creator of that account chooses the supply through its owner bytes. Every caller binds the slot to the lp_mint key of the pool, so we found no path.
- **The allowance value cap counts a lender's locked shares** — `shortwin-math::allowance` · NEW — Code smells: by_free divides T among the unlocked shares only, but by_value caps at the value of all shares; withdraw_in checks amount <= allow, not shares <= u; a deposit adds unlocked shares to a position that already holds a lock; deposit and withdraw run in one transaction (no cooldown). — A locked lender deposits one token for fresh unlocked shares, then withdraws all free tokens, because the value cap counts its locked shares and withdraw_in checks tokens not shares. The formula is the normative ECON 9.3 `min(u*T/su, value of shares_i, T)` (pool_model matches, pool_vectors would need the same change) and the over-withdrawal is the disclosed KI-13 run residual whose fix is the deferred O10 withdrawal queue; whether it truly exceeds the accepted +11.1% bound is unverified.
- **check_sim allows a minimum rent above the cr_open to cr_liq gap** — `shortwin-math::check_sim` · NEW — Code smells: TierCrGap (params.rs:1201-1207) requires only cr_open_bps ≥ cr_liq_bps + 2,000 and cr_liq_bps ≥ 10,500. No rule ties rate_base_bps, rate_slope1_bps, rate_slope2_bps and min_rent_s to that gap, and the compiled bounds allow 50,000, 1,000,000, 1,000,000 and 86,400. R_due = max(accrued, r_min) charges all of r_min from the first second (open_short.rs:583-584, liquidate.rs:333-334). The ECON scan tests each extreme alone, not combined. — Governance can install in-bounds tier terms whose minimum rent exceeds cr_open − cr_liq, so a keeper liquidates each new position at once. The default terms are safe, and we checked no deployed configuration.
- **A keeper earns more from a later liquidation than from expire.** — `shortwin-math::expire` · NEW — Code smells: `waterfall::expire` pays `k_e = risk::k_e(x.b_ref, keeper)?.min(w)`: K_e = min(0.001 SOL + 0.1 % of B̂_ref, 0.05 SOL, the remainder) (`ExpiryKeeperTerms::DEFAULT`). When the same expired position becomes CR-liquidatable, `liquidate` admits it (`a.cr_liquidatable() || !a.expired`, liquidate.rs:441) and pays kp + top ≥ K_s = min(0.01 SOL + 1 % of V, 0.25 SOL) while the borrower has equity. Example: V = 20 SOL and B̂_ref ≈ 13 SOL give K_e ≈ 0.014 SOL and K_s = 0.21 SOL. `expire` stays open to the keeper later, so a delay costs it nothing. ECON §8.4 gives the K_e formula and no incentive analysis, unlike §8.5 for liquidation. A position that stays open past `release_at` breaks the lock horizon that `Market.max_release` gives the lenders (I6). — expire pays a keeper at most 0.05 SOL, and a later liquidation pays up to 0.25 SOL, so a keeper gains from delay. We did not prove a lender loss while the in-house keeper runs.
- **Each venue read copies every pump-fees tier onto the heap** — `shortwin::math_tiers` · NEW — Code smells: pump_curve::pricing_inputs collects the curve FeeConfig tiers (C2) into a Vec (pump_curve.rs:446) and the PumpSwap FeeConfig tiers (C5) into a second Vec, which Rc::from copies again (pump_curve.rs:506). pump_amm::read_state collects the P4 tiers on every read (pump_amm.rs:701). The external pump-fees admin sets the tier count; only the 4,097-byte FeeConfig allocation bounds it, at about 100 tiers of 40 bytes. open_short reads the venue twice. The program uses the default 32 KiB bump heap, which never frees. FeeTiers::select already reads the tiers in place, with no copy. — Each venue read copies every pump-fees tier onto the 32 KiB heap, so a long tier list can make curve instructions fail. The tier lists are short today (one curve tier, about 25 PumpSwap tiers), and we did not measure the heap margin.
- **A one-unit position stops the last lender's vest flush** — `shortwin-math::withdraw` · NEW — Code smells: withdraw_in pays the unreleased vest only if `shares == p.total_shares`; a 1,000-share dust deposit makes that test false for every later exit; after that exit the vest releases to S, now only the dust shares; the withdraw.rs min_sol note reverts only a deposit that lands between the quote and the exit, not a position that already stands. — Any caller keeps a one-unit position, so the last real lender is never last and its exit leaves the unreleased vest to that dust position. The disclosed KI-09 may accept this (a lender keeps its part by waiting until vest_end), and we did not verify the web warns the last lender or whether the owner counts this case under KI-09.
- **Raydium exits still reserve the pump ert of a carried position** — `shortwin::max_in_liq` · NEW — Code smells: open_short.rs:487 locks `ert = venues::effective_ert(kind, exec_rent_tol)`: exec_rent_tol on PumpCurve and PumpSwap, 0 on Raydium. pump_curve::sync moves a curve that completed with zeroed reserves and no canonical pool to None, and an execute_pool_switch window then carries the position, byte for byte, onto a Raydium pool. The exits still subtract pos.ert: bounds::max_in_liq and max_in_exp (`V − K_s − ert`), close_max_in (`V − R − ert`), the close check `B̂ + R + ert ≤ V`, and Budget::exceeded_by (`B̂ + ert > V − K_s`), so the S1 legs arm ert lamports early. raydium::direct_buy spends no ert. — Exits of a curve position carried onto Raydium still reserve the pump ert, so its borrower needs up to ert more lamports to close. This assumes ARCH §2.4 rule 3 (ert 0 on Raydium) outranks §2.6 rule 4 (positions keep their locked terms); the loss is at most ert.
- **A burn before open_short raises the fee that sizes the caps.** — `shortwin::open_short` · NEW — Code smells: pump_amm::read_state builds PammFeeSchedule { base_supply: s } from the live mint supply (pump_amm.rs:702-709). It resolves fee_ema with the same schedule at quote = ⌊P_ema·base/WAD⌋ (pump_amm.rs:715-723). open_short passes Some(ctx.accounts.mint.supply), which a burn earlier in the transaction lowers (open_short.rs:257-263). VenueState::cap_fee_ppm takes min(φ_live, φ_ema), and the burn raises both (venues/mod.rs:796-807). open_short uses it for check_open (open_short.rs:383) and the min_open_fee_ppm mark (open_short.rs:420). Near the 98,240 SOL threshold, a burn of 0.01-0.07 % of supply moves φ from 30 to 33 bps, and fee_size_cap and fee_oi_cap grow 10 %. KI-72 discloses the burn for the exit risk view only. — A holder who burns tokens before open_short lowers the PumpSwap market cap at P_ema, so cap_fee_ppm rises a tier and the fee-scaled caps grow. We did not prove that the larger position pays back the burned tokens.
- **open_short skips the ema_warm check on curve opens.** — `shortwin::open_short` · NEW — Code smells: open_short requires market.ema_warm == 0 only inside the Raydium branch (open_short.rs:300) and the PumpSwap branch (open_short.rs:317). The PumpCurve branch (open_short.rs:319-334) has no such check. keeper_exit requires it on every venue (liquidate.rs:575). state/market.rs:291-295 says that opens, liquidate and expire refuse EmaWarmingUp while ema_warm is positive on an executable venue. create_market uses observe::initial, and crank_price redetect uses Observation::new or rewrite_liquidity; neither marks a restart on the curve. — open_short checks ema_warm only on Raydium and PumpSwap, while the Market doc and keeper_exit apply it on every executable venue. No instruction gives a curve market ema_warm above 0 today.
- **The open_short deny list omits the trader's token accounts.** — `shortwin::open_short` · NEW — Code smells: The deny array holds config, venue_guard, oracle_cache, market, market_vault, pool_vault, trader, trader_wsol, position, exposure and borrower (open_short.rs:233-245). close_short.rs:148 and liquidate.rs:533 hold the same set without the oracle and the latch. market.trader_base_ata and ATA(trader, WSOL) (pump_curve::trader_wsol_ata) are absent. parse_extras appends the extras to the CPI that invoke_as_trader signs with the trader seeds. pool_denylist covers only the pool and its reserve vaults. ARCH §2.4 rule 4 says that the deny list bounds a wrong or hostile Emergency write. — The open_short deny list omits trader_base_ata and the trader's WSOL ATA, so a wrong Emergency extras set can pass them to the CPI. Only the Emergency or Governance multisig writes the extras, so no path exists today.
- **open_short moves the sell refund with only a rent floor.** — `shortwin::open_short` · NEW — Code smells: The refund uses crate::lamports::move_lamports, which calls move_lamports_above with reserved = 0, so the only floor is rent_min(384) (open_short.rs:650-654; lamports.rs:26-28). SellOut.refund is a per-venue constant (the WSOL rent) or pump_amm::sell's wsol_rent + venue_rent, not a measured balance. No post-condition of open_short checks I2 (position lamports ≥ rent_min + v) after the move. close_short, liquidate and the settlements pay out of v. — open_short moves SellOut.refund to the borrower with only the rent floor, so a wrong adapter refund gives the borrower lamports that back v. The refunds are exact today.
- **open_short prices the post-open state on the entry mint supply.** — `shortwin::open_short` · NEW — Code smells: Step 9 calls venues::read_state_boxed(.., Some(accs.mint.supply)) after venues::sell (open_short.rs:511-520). accs.mint is an InterfaceAccount<Mint> that Anchor loaded at entry, and nothing calls reload(). The PumpSwap tier reads mcap = quote·supply/base, and the curve's synthetic fee reads the live supply. keeper_exit and sol_settle read fresh bytes through venues::mint_supply. The mint filter allows only metadata and group extensions. — open_short reads accs.mint.supply as Anchor loaded it before the sell CPI, so a sale that burns tokens prices N_open on an old supply. No sell leg burns tokens today.
- **open_short reads the VenueGuard latch without a version check.** — `shortwin::open_short` · NEW — Code smells: The two writers check the layout version: flag_venue_upgraded.rs:54 and revalidate_venue.rs:51 require venue_guard.load()?.version == VENUE_GUARD_VERSION. The four readers bind venue_guard by seeds and bump only: open_short.rs:121, create_market.rs:200, propose_pool.rs:114 and execute_pool_switch.rs:92. They then read upgraded through guard::latch, guard::candidate_latch or guard::candidate_latched. Every instruction that takes GlobalConfig or Market checks its version. AIR-01 already took guard_len and guard_elf from _reserved. — open_short binds venue_guard by seeds and bump only, so after a VenueGuard layout change, opens read the latch from the wrong bytes and pass. Only VENUE_GUARD_VERSION = 1 exists today, so no path exists now.
- **open_short computes the D4 margin at the live PumpSwap fee.** — `shortwin::open_short` · NEW — Code smells: The caps take phi_ppm = state.cap_fee_ppm() = min(φ_live, φ at P_ema) on the pre-trade read (open_short.rs:383). n_open = post.buy(q) and b_ema = post.buy_at(q, ema_xy) use the post-open read at the live fee (open_short.rs:521-522). m = risk::margin_d4(s, n_open, b_ema, cr_open) (open_short.rs:543). pump-fees charges less at a higher market cap, and a buy before the open raises it. The ring's cached (x, y) does not move, so b_ema falls by about Δφ·n. Adjacent canonical tiers differ by about 5-10 bps, so at cr_open = 30,000 the margin falls by about 0.15-0.3 % of the notional. — A borrower who buys before open_short lowers the PumpSwap fee tier, so open_short computes the D4 margin at that lower fee. We did not prove a margin cut above about 0.3 % of the notional.
- **open_short takes no bound on the locked minimum rent.** — `shortwin::open_short` · NEW — Code smells: util_pre comes from the pool counters at execution (open_short.rs:404), after any earlier open in the slot. r_min = rent::min_rent(q, p_ema, rate, util_pre, kink, min_rent_s) is locked on the position (open_short.rs:583-584). Every exit charges max(accrued, r_min). open_short gives the borrower max_margin, max_open_fee and min_proceeds, but no bound on r_min. At the default Tier D terms, 89 % utilisation adds about 0.36 % of the notional; at extreme in-bounds terms it adds up to about 24 %. — An earlier open in the same slot raises the utilisation that prices r_min, and the borrower has no argument to bound it. We did not prove a profit for the front-runner, a lender who gets part of the higher r_min.
- **open_short takes no bound on the tier it assigns.** — `shortwin::open_short` · NEW — Code smells: The tier comes from engine_tier::assign at execution (open_short.rs:360), on R_eff = min(R_now, R_slow, locked/λ) and the oracle cache. An old or empty cache gives tier C. The only user bounds are min_proceeds, max_margin and max_open_fee (open_short.rs:202-210). Nothing bounds the locked rate_* (the tier C base rate is 2× tier B), cr_liq_bps, penalty_bps or max_duration_s (7 d against 14 d). A sale inside the 10 % spot-EMA band cuts R_now by about 5 %. A tight max_margin catches the change only because the default cr_open differs per tier. — An attacker who lowers R_now before the open moves the borrower to tier C terms, because open_short takes no bound on the tier. We did not prove that the attacker profits from the worse terms.
- **open_short applies the real-SOL floor on Raydium only.** — `shortwin::open_short` · NEW — Code smells: The Raydium branch requires engine_tier::r_eff(..) ≥ pool_min_real_sol (PoolTooThin, open_short.rs:301-307). The PumpSwap branch checks only disable_flags and ema_warm (open_short.rs:309-318). engine::tier::r_eff already computes R_eff for PumpSwap from the burned LP's depth (M6R-1). ARCH §2.6 rule 5 says that opens require R_eff ≥ pool_min_real_sol and that PumpSwap reads the same terms. ARCH §3.4 step 3 lists the floor under Raydium only. The caps on X₀ shrink with the pool. — A borrower opens on a PumpSwap pool below pool_min_real_sol, because open_short checks R_eff against the floor only on Raydium. We assume the floor applies to every pool venue, as ARCH §2.6 rule 5 states for opens.
- **open_static checks only the curve bit on a curve open.** — `shortwin::open_short` · NEW — Code smells: open_static reads route_broken(config, market, kind) and config.venue_enabled & kind.venue_bit() (guard.rs:93, 102). venue_bit(PumpCurve) is bit 0 only. guard::latch uses guard_mask(PumpCurve) = pump | pfee | pAMM, and both Raydium kinds use the mask of both Raydium programs, because a position may exit there later (R4B-7). venue_enabled is read only at guard.rs:102. After the curve completes, pump_curve::sync and pump_amm::graduate move the market to PumpSwap. Exits read neither venue_enabled nor the route-broken flag. — A borrower opens a curve position after the Emergency disables PumpSwap or flags its route, because open_static checks only VENUE_BIT_PUMP_CURVE. We assume that the PumpSwap bits must also gate curve opens, as the latch mask does.
- **The extras deny lists omit trader_base_ata** — `shortwin::parse_extras` · NEW — Code smells: The deny arrays of open_short (open_short.rs:233), close_short (close_short.rs:148) and keeper_exit (liquidate.rs:533) name market.trader_wsol but not market.trader_base_ata. pool_denylist (mod.rs:1181) covers only the reserve accounts of the venue. The token program owns trader_base_ata, so it passes every parse_extras check. The trader PDA is its authority and signs the venue CPI with the extras appended, and during a sell leg the account holds the q borrowed tokens. — An Emergency extras entry can name trader_base_ata writable in the venue CPI that the trader PDA signs, because no deny list holds it. No pump or PumpSwap instruction reads a trailing account today.
- **parse_extras rejects a read-only extra that the transaction passes writable** — `shortwin::parse_extras` · NEW — Code smells: mod.rs:218-219 checks `let writable = entry.writable == 1; require!(info.is_writable == writable, ShortwinError::BadExtras);`. For a top-level instruction the runtime sets is_writable per key for the whole message. The venue lists already pass some keys writable: the PumpSwap T10, T11, T17, T18 and trader_base_ata, and the curve T4, T5, T6 and T8. check_entry (mod.rs:297) cannot see the conflict when the Emergency multisig writes the set. The CPI meta uses the entry flag, so a writable entry grants no new privilege. Callers can still select none, previous or stable. — parse_extras requires the message writability to equal the entry flag, so a read-only entry for a key that the venue passes writable always fails. Only an Emergency set_venue_extras write creates such an entry, and we found no published set with one.
- **A PumpSwap config that does not parse blocks every live curve exit.** — `shortwin::pricing_inputs` · NEW — Code smells: `pricing_inputs` reads C5 with `pump::read_fee_config(..).ok()?` and C6 with `pump_amm::read_global_config(..).ok()?`, for a live curve and a complete curve alike; `read_global_config` refuses a wrong owner or discriminator, fewer than 949 bytes and a byte at 417, 642, 939 or 940 that is not 0 or 1; `read_state` sets `executable` only when `venue.pricing.is_some()`, and `exit_venue_checks` refuses `NoRoute` when `executable` is false (close_short, liquidate, expire); every pump read passes the mint supply, so live quotes use `curve_synthetic`, only `open_short`'s `GraduationHalted` gate reads the C5 / C6 fields, and `buy_v3` takes neither account (`TRADE_CPI_ORDER`); liquidatable positions then settle S3 at the `cap_only` FV after `s3_wait_s`, and healthy positions wait for their deadline (KI-43). — When the PumpSwap FeeConfig (C5) or GlobalConfig (C6) does not parse, pricing_inputs returns None, so close_short, liquidate and expire fail NoRoute on every live curve. No live curve exit or quote reads them; we did not prove that a pAMM or pump-fees upgrade changes their layouts.
- **post_complete_known accepts every curve whose ivqr is zero.** — `shortwin::pricing_inputs` · NEW — Code smells: `post_complete_known: !post_complete || ((pcb != 0) == (pcq != 0) && curve.quote_offset_consistent())` (pump_curve.rs:461-462); `BondingCurve::quote_offset_consistent` returns true at once when `initial_virtual_quote_reserves == 0` (shortwin-venue-state pump.rs:261-262); pump never backfills `ivqr`, also when a v3 trade grows an older curve to 166 bytes (pump_math_it: 'the growth leaves ivqr 0'); `migrating_fv` / `migrated_pool` then add `pcQ` on top of `rQ` with no check. — quote_offset_consistent returns true when ivqr is 0, which pump leaves on every curve created before 2026-10-08, so FV skips its V5 check there. Today pump keeps vQ − rQ on such curves; a later pump change that books pcQ into rQ makes S3 settle at a wrong FV.
- **Pool selection ignores the candidate's venue_enabled and venue_route_broken flags** — `shortwin::propose_pool` · NEW — Code smells: propose_pool gates the candidate only with guard::candidate_latch (propose_pool.rs:235), and execute_pool_switch only with candidate_latched (execute_pool_switch.rs:170-171). config.venue_enabled is read only in guard::open_static (guard.rs:102), and config.venue_route_broken only in route::route_broken (route.rs:37). decide adopts at once on a market with no open position (pool_select.rs:299-307), so opens on that market stop until a timelocked EnableVenues. A window carries open positions onto the flagged venue, and pool_select::adopt sets no_route_since = 0 (pool_select.rs:615), so the next prologue starts the S3 chain again; a deeper candidate on a flagged venue also replaces a candidate on a working venue (d_cand > fp_d). — Any caller moves a market, and any open positions, onto a pool of a venue that the Emergency disabled or flagged route-broken. This assumes the flags must gate candidates, but ARCH §2.3.2 limits disable_venue to opens; we found only blocked opens and an S3 restart.
- **VenueGuard readers skip the VENUE_GUARD_VERSION check** — `shortwin::propose_pool` · NEW — Code smells: flag_venue_upgraded.rs:54 and revalidate_venue.rs:51 constrain `venue_guard.load()?.version == VENUE_GUARD_VERSION`. propose_pool.rs:114, execute_pool_switch.rs:92, create_market.rs:200 and open_short.rs:121 have only `seeds = [VenueGuard::SEED], bump = venue_guard.load()?.bump`. Every reader of GlobalConfig and Market checks CONFIG_VERSION or MARKET_VERSION. — create_market, propose_pool, execute_pool_switch and open_short read VenueGuard.upgraded with no VENUE_GUARD_VERSION check, while its writers check it. After a layout change, a reader that the upgrade misses tests other bytes and ignores a set upgrade bit.
- **propose_pool lists a Raydium pool while the other Raydium program awaits review** — `shortwin::propose_pool` · NEW — Code smells: create_market.rs:547 applies guard::latch(.., VenueKind::RaydiumV4), the whole GUARD_MASK_RAYDIUM, to every Raydium listing ('a listing is never urgent', M3-SEC-2). propose_pool.rs:235 applies candidate_guard_mask(cand_kind), the candidate's own bit, to every proposal, Adopt(POOL_KIND_LISTING) on a None market included. guard.rs:118-128 gives the fast path as the only reason for the narrower mask; a None market with no open position adopts as a listing, not as a fast path, and fixes the pool, its SOL side and graduated_at. — Any caller lists a Raydium pool through create_market(Auto) and propose_pool while the other Raydium program awaits review, which create_market with a Raydium hint refuses. Opens still check both programs, so no harm exists today; a later rule that relies on the listing latch does not hold here.
- **A pool switch keeps graduated_at from a thin old pool** — `shortwin::propose_pool` · NEW — Code smells: pool_select::adopt writes graduated_at only for a market that comes from None (pool_select.rs:600-602), so a Raydium-to-Raydium adoption keeps the old age. raydium_cpmm::verify counts the age from open_time, the whole life of the pool and its thin months too (raydium_cpmm.rs:424). create_market judges a Raydium listing at the candidate's own spot, which one transaction can move (create_market.rs:561-568; KI-17 (a)). decide adopts at 5 % over D_A inside the listing window (pool_select.rs:299-307). tier.rs:372-385 then gives Tier A from the age of pool A and the depth of pool B. The S3 grace after graduated_at also ends early on the new pool. — Any caller lists a market on an old thin CPMM pool and adopts a deep new pool, so borrowers get Tier A terms within hours. This assumes the age must follow the pool whose locked depth sets the tier, as SPEC O13b assumes; we did not prove a lender loss.
- **pump_cashback_claimable reads a UVA with a foreign owner as empty** — `shortwin::pump_cashback_claimable` · NEW — Code smells: `if *uva.owner != PUMP_PROGRAM_ID { return Ok(0); }` (sweep_trader.rs:130-132) treats every other owner as nothing claimable, not only a System account that was never created. The sibling reader pump_amm::cashback_claimable (pump_amm.rs:791-802) returns u64::MAX for an unexpected owner. A 0 makes rewards_clear true, and while S = 0 the VenueRewards sweep sets wipe_rewards_claimed (sweep_trader.rs:340-352, 377-390). — pump_cashback_claimable returns 0 for every owner that is not pump, so sweep_trader can set wipe_rewards_claimed with no proof that the UVA is empty. Only a pump upgrade can give the UVA another owner.
- **Raydium parse reads the vaults with no check of their token authority.** — `shortwin::raydium_cpmm::parse` · NEW — Code smells: `raydium_cpmm::parse` (raydium_cpmm.rs:186-190) and `raydium_v4::parse` (raydium_v4.rs:160-164) check the token program that owns each vault and its mint. They do not check the token authority `t.owner == RAYDIUM_CPMM_AUTHORITY` / `RAYDIUM_V4_AUTHORITY`; only the adoption-time verify does (raydium_cpmm.rs:393-401, raydium_v4.rs:304-308). `pump_amm` checks its vault owner on every read (pump_amm.rs:604). On the Raydium trade path, the reserves that set every quote, max_in, B̂_ref, pool_d and the S3 and pool_fail chains rest only on the pool's own vault fields and the keys cached at adoption. This holds today because only the venue's authority can re-assign its vaults. — The Raydium read path prices on the vault balances and does not check that the pool authority owns each vault. Only the adoption check and the cached vault keys bind it today.
- **Graduation drops the ρ factor that the migrating FV applies to P_ema.** — `shortwin::restart_observation` · NEW — Code smells: On `PumpMigrating`, `Quoter::fv_repriced` prices FV with `migrating_fv`, which scales the price by ρ (`p' = ceil(ceil(p*Y1/Y0)*X0/X1)`) after a pool part; at `-> PumpSwap`, `restart_observation` rewrites only the liquidity (`rewrite_liquidity(anchor, ..)`) and marks the warm-up, so `P_ema` and the ring prices keep the curve's values; `settle::evaluate` skips only `inp.executable && !inp.warmed`, so `sol_settle` evaluates S3 on a pool that cannot execute, with FV `buy_repriced(q, L_anchor, P_ema)`; the 98u8RJ fixture gives ρ = 1.0631. — After a synthetic completion, restart_observation keeps the curve P_ema, so an S3 on a halted PumpSwap pool prices FV about 6 % low. The borrower keeps the gap; we did not show a pump halt that starts at graduation.
- **revalidate_venue accepts only the exact slot for an ELF above 2 MiB.** — `shortwin::revalidate_venue` · NEW — Code smells: `trimmed_elf_hash_of` returns `None` when the trimmed ELF is longer than `ELF_HASH_MAX_LEN`, 2 MiB (flag_venue_upgraded.rs:94). `is_upgrade` counts `None` as an upgrade (flag_venue_upgraded.rs:107). `revalidate_venue` accepts a moved slot only if `trimmed_elf_hash(..)? == Some(expected_elf_hash)` (revalidate_venue.rs:79), and `None` never equals it. In Agave 4.2.2, `ExtendProgram` calls `common_extend_program(.., false)`: it takes no signer, runs once per program per slot, adds at least 10 KiB and moves the deploy slot. pump-fees (guard 2) is in `GUARD_MASK_PUMP_CURVE` and `GUARD_MASK_PUMP_SWAP`. The 10 MiB cap ends the extensions after about 800 rounds. KI-69 (e) records the 2 MiB cap as an accepted residual. — If a fingerprinted venue's ELF grows above 2 MiB, any caller who extends its ProgramData before each revalidate_venue makes it fail, so opens stay blocked. Today the ELFs of pump-fees, AMM v4 and CPMM are 0.31–0.85 MB, so the path is not live.
- **revalidate_venue records the expected hash unchecked at a matching slot.** — `shortwin::revalidate_venue` · NEW — Code smells: `revalidate_venue` hashes the trimmed ELF only when `slot != expected_slot` (revalidate_venue.rs:75-82). At a matching slot, line 106 records `(len, elf_fingerprint_of(&expected_elf_hash))` as given, and line 108 clears the `upgraded` bit. The module docs (revalidate_venue.rs:19-21) and KI-69 (c) call a wrong hash fail-safe; that is true only when the named slot holds the reviewed ELF. The hash costs up to about 0.44 M CU (KI-69 (e)), and `shortwin-gov` requests no compute limit. — At a matching deploy slot, revalidate_venue does not hash the ELF, so the Emergency clears the latch even when expected_elf_hash names another ELF. The path needs an Emergency hash that does not come from the account, and shortwin-gov takes the hash from the account (KI-69 (c)).
- **A PumpMigrating market without pricing settles S3 at ρ = 1.** — `shortwin::risk_quoter` · NEW — Code smells: `risk_quoter` sends every `PumpMigrating` state without `CurvePricingInputs` to `cap_only(cap_bps)`; `Quoter::fv_repriced` then prices FV on `reprice(L_ema, P_ema)` of the frozen curve at ρ = 1 and the hard-cap fee; the documented exception (pump-final 6.1.5) makes FV `None` only when pricing parses and `post_complete_known` is false; the curve's own `pcB` / `pcQ`, which `read_curve` always decodes, are not checked when pricing is `None`; with E1bY's pool part (ρ = 6.4556) FV is about 1/6 of the post-migration pool's value. — risk_quoter prices a PumpMigrating market with no pricing inputs at cap_only, so an S3 after a pool part takes FV at ρ = 1. No outside caller breaks those accounts today; a layout change of pump, pump-fees or pAMM during a stuck migration opens it.
- **The sol_settle no-op stores MarketVault only when three fields changed** — `shortwin::sol_settle` · NEW — Code smells: evaluate snapshots tracked_before = (lender_sol, unmatched, protocol_fees) (sol_settle.rs:220). Its None branch calls set_vault only when that tuple changed (sol_settle.rs:275). pool::Vault also holds pi, lock_norm, b_min, pi_epoch, cal_day and lock_cal. keeper_exit and close_short always store the vault after the prologue. pool::accrue writes only lender_sol and protocol_fees today. — A later prologue step that writes another MarketVault field keeps its effect in every other instruction but loses it in a no-op sol_settle. Today the prologue writes only the three tracked fields.
- **sol_settle passes market.token_program to mint_guard as the token program** — `shortwin::sol_settle` · NEW — Code smells: sol_settle.rs:196-201 sets token_program = market.token_program and calls mint_guard(&market, &mint, &token_program, MintPath::Exit). guard.rs:49-53 then requires *token_program == market.token_program, so it compares the field with itself. The second check, mint.owner == market.token_program, still binds the mint. — The third key check of mint_guard always passes in sol_settle, which has no token program account. A later token CPI in sol_settle that relies on mint_guard gets no token program check.
- **A same-transaction buy lowers the PumpSwap fee in sol_settle's fair value** — `shortwin::sol_settle` · NEW — Code smells: risk_quoter returns state.quoter().capped(cap) (venues/mod.rs:1138-1139), and FeeBps::capped keeps a live fee below the cap. The quoter reads the live PumpSwap tier on the post-trade reserves (pump_amm.rs:710). exit_risk_cap_bps caps only from above (venues/mod.rs:822-826), and its doc says that a same-transaction trade can only lower the risk fee. FV = B̂_FV + ⌈p'·B̂_FV/10⁴⌉ + R_due uses that fee, so x = min(V - K_s, FV) falls. The agent's numbers: FV falls 0.69 % (12,772,001 lamports) at the 95 → 25 bps step, and the buy and sale cost about 1,007,824 lamports. — A borrower who buys tokens before its own sol_settle in one transaction lowers the PumpSwap fee, so its S2 or S3 pays the lenders less. We did not prove that a pool with the 70-bps step sits at the 420 SOL threshold while an S2 or S3 is due.
- **sol_settle times a position's S3 wait from the market's lower-cap chain** — `shortwin::sol_settle` · NEW — Code smells: observe_chains starts no_route_since with s3_no_route at chain_hard_cap_bps = min(global cap, min_open_hard_cap_bps) (route.rs:43-59). s3_chain copies that start into every position's chain (arming.rs:181). s3_due counts s3_wait_s from it (settle.rs:357-376). Only no_route_now uses the position's own cap (route.rs:65-78). KI-74 accepts a mark that a closed position set. Here the low-cap position is still open, so exact tracking gives the same start. The trace needs Governance to raise venue_fee_hard_cap_bps between two opens and a venue fee between the two locked caps. — When a lower locked cap starts the market chain, a higher-cap position reaches S3 in the second the fee passes its own cap. We assume s3_wait_s is meant to count from that second, and we did not measure the borrower's loss.
- **An AMM v4 deposit moves PnL into need_take_pnl and lowers locked_depth** — `shortwin::state_of` · NEW — Code smells: raydium_v4.rs:180-182 computes `D = floor(lambda * isqrt(X*Y) / 10^4)` with `X`, `Y` = vault - `need_take_pnl` (`AmmInfo::reserves`); raydium.rs:210 says `D` moves with neither deposits nor withdrawals, and `execute_pool_switch` judges a window on the terms no transaction can move (M4-SEC-1 / M4-SEC-3); docs/research/raydium-amm.md section 2.2 records that `deposit`, `withdraw` and `withdrawpnl` raise `need_take_pnl` through `calc_take_pnl`, by 12/100 of the constant-product growth since the `calc_pnl_x/y` snapshot; `execute_pool_switch` verifies the candidate again in the same transaction and cancels the window when `window_floor_ok` or `carried_fit` fails (execute_pool_switch.rs:172-189); the same deposit before `propose_pool` lowers `D_inc`, which the prologue refreshes; the research note calls the pending share negligible (raydium-amm.md section 8; TX-V4-PNL took 0.0002 % of reserves; need_take_pnl is 0 on the six target pools); CPMM is not affected, because its fee counters are already outside the reserves. — A caller who deposits a small amount into an AMM v4 pool before execute_pool_switch lowers its locked_depth once, by the protocol's pending fee share. We did not measure the drop on a real pool or find a window that it cancels.
- **store_records writes 16 bytes into accounts that it does not check** — `shortwin::store_records` · NEW — Code smells: store_records (lender.rs:88-113) pairs the accounts with before and after by position. It checks only is_writable (:102) and then copies conv_rem into bytes 127..143. load_records (:48-79) checks the owner, discriminator, market, address and epoch order, but returns plain records, not the checked accounts. The zip stops at the shorter list, so an accounts slice shorter than before drops a conv_rem write with no error and breaks the KI-65 sum. All five callers pass ctx.remaining_accounts to both functions today (deposit.rs:134/151, withdraw.rs:190/215, claim_sol.rs:84/91, cover_deficit.rs:69/74, sync_lender.rs:113/134). sweep_trader already splits its remaining accounts before load_records. — store_records writes conv_rem at byte 127 of each changed account and checks only that the account is writable. A future caller that passes another slice overwrites 16 bytes of any writable account that the program owns.
- **A donation blocks deposits if claim_cashback ever needs the trader's signature** — `shortwin::sweep_trader` · NEW — Code smells: The venue-reward bit has one setter, behind rewards_clear (sweep_trader.rs:248-264, 340-352, 377-390). Anyone can raise the claimable balance with a donation to the pump UVA or to ATA(pAMM UVA, WSOL). Only the unsigned claim_cashback of the venue lowers it, and short.win signs no claim for the trader. No timeout, Emergency path or Governance path sets the bit. deposit_in returns WipeSweepPending while S = 0 (pool.rs:1983-1995), and the exit of the last lender also reaches S = 0 (pool.rs:1866-1870). sweep_trader.rs:53-55 says that no change of pump's claim interface can lock deposits. — If a venue upgrade makes claim_cashback need the trader's signature, a donor blocks every deposit into a pool with no shares. We did not prove that pump or PumpSwap makes this change, or one that pays out only tracked cashback.
- **sync_after_leg passes CanonicalPool::Absent and never reads C3.** — `shortwin::sync_after_leg` · NEW — Code smells: `sync_after_leg` builds `CurveVenue { canonical_pool: CanonicalPool::Absent, pool: None, pricing: None, .. }` (pump_curve.rs:733-738), although C3 (`market.canonical_pool`, key-checked) is in `read`; `sync` maps `!c.canonical_pool.exists() && c.live.vt == 0` to `VenueKind::None` and sets `MARKET_FLAG_GRADUATED_ELSEWHERE` (pump_curve.rs:638-649), while `read_state` reads C3 through `detect::canonical_pool_read` for every complete curve (M5-SEC-5); `.ok().flatten()` drops any error that `sync` returns; the callers are `close_short` (close_short.rs:312) and `keeper_exit` (liquidate.rs:697), after the buy CPI; the next `crank_price` redetect moves such a market back, and one agent notes that a `None` market accepts a Raydium adoption through `propose_pool`. — sync_after_leg passes CanonicalPool::Absent to sync, so a buy leg that leaves vT at 0 moves the market to None with graduated_elsewhere set. No path reaches this today, because buy_v3 leaves vT above 0; a pump change that migrates inside the buy opens it.
- **sync never graduates a migrating market whose curve no longer parses** — `shortwin::sync` · NEW — Code smells: read_state returns at pump_curve.rs:536 when parse_curve fails, before it reads C3 at :543, so state.curve is None and the canonical pool is never read. sync then returns Ok(None) at :625. detect::canonical_pool_read needs only C3, the mint and the cached pool authority. propose_pool refuses every pump kind, and crank_price re-detects only VenueKind::None markets. The sync doc says that a curve that does not parse changes nothing (A63). — If pump closes a completed bonding curve or changes its layout, the market stays PumpMigrating and never moves onto its valid PumpSwap pool. We did not prove that pump does either.
- **Any caller's sync_lender advances the lock calendar at the raw clock** — `shortwin::sync_lender` · NEW — Code smells: sync_lender.rs:104 (also claim_sol.rs:76 and cover_deficit.rs:61) passes clock.unix_timestamp to pool::touch_pot and pool::touch, and their cal_advance moves MarketVault.cal_day. Market is read-only there, so last_accrual_ts does not move and a later withdraw at market_now can run behind cal_day. lock_of (pool.rs:1715-1720) never compares the day of the lock with cal_day. withdraw_in with add == 0 calls cal_add on a dropped day (pool.rs:1843-1849) and gets InvalidInput (pool.rs:833). Both agents reproduced the revert in a scratch copy of shortwin-math. sync_lender needs no signer. — Any caller's sync_lender advances cal_day at the raw clock and lock_of ignores cal_day, so after a clock step back a lock holder's withdraw fails. Agave's update_clock prevents the step back today.
- **sync sets no graduated_at when a PumpMigrating market moves to None** — `shortwin::sync` · NEW — Code smells: On a move to None, sync sets only MARKET_FLAG_GRADUATED_ELSEWHERE (pump_curve.rs:648-649) and leaves graduated_at at 0. s3_chain reads migrating from the venue after sync (arming.rs:183), so the migration_seen_at + max_migration_pause_s term of s3_due drops out (settle.rs:366-371). no_route_since dates from the completion, because a PumpMigrating curve is not executable. sol_settle skips the evaluation only for a move to PumpSwap (sol_settle.rs:262-264). The test a_curve_that_graduates_elsewhere_goes_list_only_and_settles_s3 moves the market to None 1 s after the completion. — When pump moves a migrating coin elsewhere, a keeper can run an S3 at once at the frozen curve price, before any pool adoption. We assume S3 is meant to wait grad_grace_s after this move, as it does after a graduation.
- **The Raydium readers treat a pool with a frozen vault as executable** — `shortwin::token_of` · NEW — Code smells: `token_of` (raydium.rs:85-90) returns `token_state(info).ok()`, and `token_account::read` accepts state 2 as `frozen: true` (token_account.rs:101-104); `raydium_v4::parse` (raydium_v4.rs:160-161), `raydium_cpmm::parse` (raydium_cpmm.rs:186-187), both `verify` functions and both `venue_executable` predicates never read `frozen`; `pump_amm::vault_of` (pump_amm.rs:603) uses the same `token_of`; the program reads `frozen` only in `base_ata_state` (trader.rs:273), which refuses a frozen trader ATA; a frozen vault makes every Raydium swap fail, so `close_short`, `liquidate` and `expire` fail while `pool_fail` and `s3_no_route` stay false; the mint filter refuses a freeze authority (mint_filter.rs:89), the extension allowlist (mint.rs:110) has no DefaultAccountState or Pausable entry, and SPL Token cannot add a freeze authority after `InitializeMint`. — If a Raydium vault is frozen, the pool stays executable while every swap fails, so no S3 or pool_fail chain starts. No path exists today, because the mint filter refuses a freeze authority and WSOL has none.
- **parse leaves the coin_creator placeholder unresolved on curve markets.** — `shortwin::venues::parse` · NEW — Code smells: `venues::parse` builds `Placeholders { coin_creator: (kind == VenueKind::PumpSwap).then(..), pool: pool_key(market, kind) }`. `pool_key` returns None for PumpCurve (mod.rs:1160-1170). `Placeholders::key` maps None to BadExtras (mod.rs:128-138). `check_entry` accepts `PLACEHOLDER_COIN_CREATOR` and `PLACEHOLDER_POOL` for every venue, the curve included (mod.rs:325-329; venue_extras.rs:60 names the placeholder 'the coin creator'). The curve already caches its creator in `Market.cached_creator` (pump_curve.rs:287). ARCH §2.3.2 lists `coin_creator` as a placeholder for every venue, and no source line says why the curve resolves none. The extras exist so that the program can follow a venue interface change without the 48 h upgrade (A41). — The Emergency key can publish a curve template with the coin_creator placeholder, but parse rejects it with BadExtras on every curve market. We did not show that pump needs such an account today.
- **token_account::read accepts a 355-byte Token-2022 multisig as a token account.** — `shortwin-venue-state::read` · NEW — Code smells: token_account::read refuses data.len() < 165 (ACCOUNT_LEN). For a longer account it checks only the Token-2022 owner and byte 165 == 2 (:95-100). It has no data.len() == MULTISIG_LEN (355) refusal, but its doc (:86) says that a multisig is refused, and mint::check refuses that length. In a Token-2022 multisig, byte 165, state @108, the COption tags, owner @32 and amount @64 are signer-key bytes. InitializeMultisig stores those bytes with no signature. The keeper (sweep.rs token_amount) and the WASM frontend use the same reader. — Any caller can create a Token-2022 multisig that token_account::read parses with an owner and amount that the caller chooses. Every current caller checks the key first, so only a future caller without a key check is exposed.
- **swap_enabled reads an AMM v4 pool at status 5 as halted.** — `shortwin-venue-state::swap_enabled` · NEW — Code smells: AmmInfo::swap_enabled maps status 5 (STATUS_ORDERBOOK_ONLY) to false through `_ => false` (:241). The reader never decodes state_data.orderbook_to_init_time. The public raydium-amm swap code sets a status-5 pool to Initialized once now ≥ orderbook_to_init_time. docs/research/raydium-amm.md §5.1 says that status 5 disables swaps, and §2.2 says that commit 27f461d deleted the orderbook code. Neither line says that the status branch is gone. Only the pool admin GThUX1 can set status 5 (SetParams param 0), and no pool has status 5 today. — When the AMM v4 pool admin sets status 5, short.win reads the pool as halted and runs the S3 and pool_fail chains. We did not confirm that the pinned AMM v4 build still swaps at status 5 after orderbook_to_init_time.
- **Only Anchor's duplicate-account check refuses pool_vault as the destination** — `shortwin::withdraw` · NEW — Code smells: `Withdraw` has no `owner_token_account.key() != pool_vault.key()` constraint, and its doc allows any account of this mint (withdraw.rs:120). Both fields are `mut` `Box<InterfaceAccount>` without `dup`, so anchor-syn 1.2.0 `generate_duplicate_mutable_checks` returns error 2040 before the handler. Without that check, `pool::withdraw` lowers T and the market PDA signs `transfer_checked(pool_vault -> pool_vault)`. SPL Token and Token-2022 accept a self-transfer and move nothing, so the `pool_vault.amount >= T` post-condition passes. A field type change, a `dup` marker or a framework change removes the only guard. Only the tokens of the lender are at risk. — Only the Anchor 1.2.0 duplicate-mutable check (error 2040) stops a lender who passes pool_vault as owner_token_account. If a later change removes that check, the lender loses its shares and the tokens stay in the vault.

---

> ⚠️ This review was performed by an AI assistant. AI analysis can never verify the complete absence of vulnerabilities and no guarantee of security is given. Team security reviews, bug bounty programs, and on-chain monitoring are strongly recommended.
