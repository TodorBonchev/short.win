# 🔐 Security Review — short.win

---

## Scope

|  |  |
| --- | --- |
| **Mode** | filename |
| **Files reviewed** | `./crates/shortwin-math/src/bounds.rs` · `./crates/shortwin-math/src/caps.rs` · `./crates/shortwin-math/src/consts.rs`<br>`./crates/shortwin-math/src/error.rs` · `./crates/shortwin-math/src/fees.rs` · `./crates/shortwin-math/src/lib.rs`<br>`./crates/shortwin-math/src/num/mod.rs` · `./crates/shortwin-math/src/num/u256.rs` · `./crates/shortwin-math/src/observe.rs`<br>`./crates/shortwin-math/src/params.rs` · `./crates/shortwin-math/src/persist.rs` · `./crates/shortwin-math/src/pool.rs`<br>`./crates/shortwin-math/src/pool_select.rs` · `./crates/shortwin-math/src/quote/cpmm.rs` · `./crates/shortwin-math/src/quote/mod.rs`<br>`./crates/shortwin-math/src/quote/pricing.rs` · `./crates/shortwin-math/src/quote/ray_v4.rs` · `./crates/shortwin-math/src/quote/reprice.rs`<br>`./crates/shortwin-math/src/quote/sol_leg.rs` · `./crates/shortwin-math/src/quote/straddle.rs` · `./crates/shortwin-math/src/rent.rs`<br>`./crates/shortwin-math/src/risk.rs` · `./crates/shortwin-math/src/settle.rs` · `./crates/shortwin-math/src/tier.rs`<br>`./crates/shortwin-math/src/waterfall.rs` · `./crates/shortwin-venue-state/src/bytes.rs` · `./crates/shortwin-venue-state/src/lib.rs`<br>`./crates/shortwin-venue-state/src/mint.rs` · `./crates/shortwin-venue-state/src/program_data.rs` · `./crates/shortwin-venue-state/src/pump.rs`<br>`./crates/shortwin-venue-state/src/pump_amm.rs` · `./crates/shortwin-venue-state/src/pyth.rs` · `./crates/shortwin-venue-state/src/raydium_cpmm.rs`<br>`./crates/shortwin-venue-state/src/raydium_v4.rs` · `./crates/shortwin-venue-state/src/token_account.rs` · `./programs/shortwin/src/constants.rs`<br>`./programs/shortwin/src/engine/arming.rs` · `./programs/shortwin/src/engine/bounds.rs` · `./programs/shortwin/src/engine/caps.rs`<br>`./programs/shortwin/src/engine/guard.rs` · `./programs/shortwin/src/engine/lender.rs` · `./programs/shortwin/src/engine/mod.rs`<br>`./programs/shortwin/src/engine/observe.rs` · `./programs/shortwin/src/engine/pool_select.rs` · `./programs/shortwin/src/engine/prologue.rs`<br>`./programs/shortwin/src/engine/route.rs` · `./programs/shortwin/src/engine/tier.rs` · `./programs/shortwin/src/engine/trader.rs`<br>`./programs/shortwin/src/engine/waterfall.rs` · `./programs/shortwin/src/errors.rs` · `./programs/shortwin/src/events.rs`<br>`./programs/shortwin/src/instructions/add_margin.rs` · `./programs/shortwin/src/instructions/apply_config.rs` · `./programs/shortwin/src/instructions/cancel_config.rs`<br>`./programs/shortwin/src/instructions/claim_sol.rs` · `./programs/shortwin/src/instructions/close_short.rs` · `./programs/shortwin/src/instructions/cover_deficit.rs`<br>`./programs/shortwin/src/instructions/crank_price.rs` · `./programs/shortwin/src/instructions/crank_sol_usd.rs` · `./programs/shortwin/src/instructions/create_market.rs`<br>`./programs/shortwin/src/instructions/deposit.rs` · `./programs/shortwin/src/instructions/disable_venue.rs` · `./programs/shortwin/src/instructions/execute_pool_switch.rs`<br>`./programs/shortwin/src/instructions/expire.rs` · `./programs/shortwin/src/instructions/flag_unsafe_mint.rs` · `./programs/shortwin/src/instructions/flag_venue_upgraded.rs`<br>`./programs/shortwin/src/instructions/initialize_config.rs` · `./programs/shortwin/src/instructions/liquidate.rs` · `./programs/shortwin/src/instructions/mod.rs`<br>`./programs/shortwin/src/instructions/open_short.rs` · `./programs/shortwin/src/instructions/propose_config.rs` · `./programs/shortwin/src/instructions/propose_pool.rs`<br>`./programs/shortwin/src/instructions/revalidate_venue.rs` · `./programs/shortwin/src/instructions/set_coverage_pause.rs` · `./programs/shortwin/src/instructions/set_pause.rs`<br>`./programs/shortwin/src/instructions/set_route_broken.rs` · `./programs/shortwin/src/instructions/set_venue_extras.rs` · `./programs/shortwin/src/instructions/sol_settle.rs`<br>`./programs/shortwin/src/instructions/sweep_protocol_fees.rs` · `./programs/shortwin/src/instructions/sweep_trader.rs` · `./programs/shortwin/src/instructions/sync_lender.rs`<br>`./programs/shortwin/src/instructions/withdraw.rs` · `./programs/shortwin/src/lamports.rs` · `./programs/shortwin/src/lib.rs`<br>`./programs/shortwin/src/mint_filter.rs` · `./programs/shortwin/src/state/config.rs` · `./programs/shortwin/src/state/epoch.rs`<br>`./programs/shortwin/src/state/exposure.rs` · `./programs/shortwin/src/state/layout_asserts.rs` · `./programs/shortwin/src/state/lender.rs`<br>`./programs/shortwin/src/state/market.rs` · `./programs/shortwin/src/state/mod.rs` · `./programs/shortwin/src/state/oracle.rs`<br>`./programs/shortwin/src/state/pod.rs` · `./programs/shortwin/src/state/position.rs` · `./programs/shortwin/src/state/vault.rs`<br>`./programs/shortwin/src/state/venue_extras.rs` · `./programs/shortwin/src/state/venue_guard.rs` · `./programs/shortwin/src/venues/detect.rs`<br>`./programs/shortwin/src/venues/mod.rs` · `./programs/shortwin/src/venues/pump_amm.rs` · `./programs/shortwin/src/venues/pump_curve.rs`<br>`./programs/shortwin/src/venues/raydium.rs` · `./programs/shortwin/src/venues/raydium_cpmm.rs` · `./programs/shortwin/src/venues/raydium_v4.rs`<br>`./services/common/src/chain.rs` · `./services/common/src/tx.rs` · `./services/keeper/src/budget.rs`<br>`./services/keeper/src/coverage.rs` · `./services/keeper/src/decide.rs` · `./services/keeper/src/fees.rs`<br>`./services/keeper/src/graduation.rs` · `./services/keeper/src/ixs.rs` · `./services/keeper/src/monitor.rs`<br>`./services/keeper/src/pda.rs` · `./services/keeper/src/pool_monitor.rs` · `./services/keeper/src/sweep.rs`<br>`./services/keeper/src/venue.rs` · `./services/keeper/src/view.rs` |
| **Confidence threshold (1-100)** | 75 |
| **Passes** | 2 (pass 1 ran 11/12 agents) |
| **Memory** | 0 records before this scan · 31 after · `b815269` |

---

## Findings

[75] **1. Correctness: The keeper checks the proposal floor at the wrong price**

`shortwin-keeper::evaluate` · Confidence: 75 · seen in 2/2 runs · NEW

**Description**
The keeper evaluates the floor at the wrong price, so it selects a pool that the proposal instruction rejects.

**Fix**

```diff
--- a/services/keeper/src/pool_monitor.rs
+++ b/services/keeper/src/pool_monitor.rs
@@ -373,7 +373,11 @@
             d: a.state.locked_depth,
             real_sol: a.state.r,
             fee_ppm: a.state.fee_ppm,
-            window_floor: pool_select::window_floor_ok(&a, params, p_ref),
+            window_floor: pool_select::window_floor_ok(
+                &a,
+                params,
+                pool_select::window_price(p_ref, &a),
+            ),
             fits: pool_select::carried_fit(market, &a, fit, p_ref, FitAt::Proposal)
                 .is_ok_and(|f| f.fits()),
         },
```

---

[75] **2. Correctness: The keeper skips pools after a floor failure**

`shortwin-keeper::plan` · Confidence: 75 · seen in 2/2 runs · NEW

**Description**
The keeper skips viable alternatives when the deepest candidate fails the proposal floor. This path works after evaluate fixes wrong-window-floor-reference.

**Fix**

```diff
--- a/services/keeper/src/pool_monitor.rs
+++ b/services/keeper/src/pool_monitor.rs
@@ -531,18 +531,14 @@
         return out;
     };
     out.best = Some((c.pool, d));
-    // M4V-1: a window's candidate also needs the window floor, which is monotone in `D` at the
-    // market's one `P_ema`: if the deepest candidate misses it, every candidate does.
+    // Each candidate has its own proposal price and must pass its own floor.
     let window_floor = c.window_floor();
     match pool_select::decide(s, d, p) {
-        Ok(Proposal::OpenWindow | Proposal::ImproveWindow) if !window_floor => {
-            out.blocked = Some(u32::from(ShortwinError::PoolTooThin));
-        }
         // Switch only if all fit: the window takes the deepest candidate whose opening caps every
         // open position fits (not monotone in `D`: the fee, the reserve and the tier count too)
         // and that `decide` still takes for the window; with none, the positions exit on the
         // failed pool through the S3 chain.
-        Ok(Proposal::OpenWindow | Proposal::ImproveWindow) if !c.fits() => {
+        Ok(Proposal::OpenWindow | Proposal::ImproveWindow) => {
             let fitting = ranked.iter().find_map(|&(d, c)| {
                 if !(c.fits() && c.window_floor()) {
                     return None;
@@ -564,7 +560,13 @@
                         expect,
                     });
                 }
-                None => out.blocked = Some(u32::from(ShortwinError::CarriedPositionsTooLarge)),
+                None => {
+                    out.blocked = Some(u32::from(if window_floor {
+                        ShortwinError::CarriedPositionsTooLarge
+                    } else {
+                        ShortwinError::PoolTooThin
+                    }));
+                }
             }
         }
         Ok(expect) => {
```

---

[75] **3. Correctness: The keeper omits supported coin creator extras**

`shortwin-keeper::resolve_entry` · Confidence: 75 · seen in 2/2 runs · NEW

**Description**
The keeper drops every extras set with the supported coin creator placeholder, so it cannot construct those configured exit variants.

**Fix (Option A — resolve the cached PumpSwap creator with an expression arm; unapplied and uncompiled)**

```diff
--- a/services/keeper/src/venue.rs
+++ b/services/keeper/src/venue.rs
@@ -672,7 +672,11 @@
                             PLACEHOLDER_POOL => pool_placeholder(kind, market),
                             PLACEHOLDER_TRADER => Some(market.trader),
                             PLACEHOLDER_BONDING_CURVE => Some(market.bonding_curve),
-                            // The coin creator is not resolvable from the market cache.
+                            shortwin::state::PLACEHOLDER_COIN_CREATOR
+                                if kind == VenueKind::PumpSwap => Some(pump_amm::cached(
+                                    market,
+                                    pump_swap_cache::CACHED_COIN_CREATOR,
+                                )),
                             _ => None,
                         }?;
                         owned.push(key.to_bytes().to_vec());
```

**Fix (Option B — resolve the cached PumpSwap creator with a block arm; unapplied and uncompiled)**

```diff
--- a/services/keeper/src/venue.rs
+++ b/services/keeper/src/venue.rs
@@ -672,7 +672,14 @@
                             PLACEHOLDER_POOL => pool_placeholder(kind, market),
                             PLACEHOLDER_TRADER => Some(market.trader),
                             PLACEHOLDER_BONDING_CURVE => Some(market.bonding_curve),
-                            // The coin creator is not resolvable from the market cache.
+                            shortwin::state::PLACEHOLDER_COIN_CREATOR
+                                if kind == VenueKind::PumpSwap =>
+                            {
+                                Some(pump_amm::cached(
+                                    market,
+                                    pump_swap_cache::CACHED_COIN_CREATOR,
+                                ))
+                            }
                             _ => None,
                         }?;
                         owned.push(key.to_bytes().to_vec());
```

---

[75] **4. Correctness: Pool ranks round positive locked SOL to zero**

`shortwin-keeper::score` · Confidence: 75 · seen in 2/2 runs · NEW

**Description**
An LP dilutes the locked share below one basis point, so the keeper excludes a pool whose locked SOL stays positive.

**Fix**

```diff
--- a/services/keeper/src/pool_monitor.rs
+++ b/services/keeper/src/pool_monitor.rs
@@ -75,7 +75,7 @@
 use shortwin::errors::ShortwinError;
 use shortwin::state::{Market, VenueKind, raydium_cache};
 use shortwin::venues::detect::{self, PumpClass};
-use shortwin_math::pool_select::{dominates, fast_path_qualifies, lambda_bps};
+use shortwin_math::pool_select::{dominates, fast_path_qualifies};
 use shortwin_services_common::{AccountData, Chain, Memcmp};
 use shortwin_venue_state::{mint, raydium_cpmm, raydium_v4};
 
@@ -249,7 +249,12 @@
     let Ok(m) = mint::read_base(&lp.owner.to_bytes(), &lp.data) else {
         return 0;
     };
-    u128::from(lambda_bps(c.lp_counter, m.supply)) * u128::from(r) / 10_000
+    match c.lp_counter.checked_sub(m.supply) {
+        Some(locked) if c.lp_counter != 0 => {
+            u128::from(locked) * u128::from(r) / u128::from(c.lp_counter)
+        }
+        _ => 0,
+    }
 }
 
 /// The best `k` candidates by score (highest first; among equals the lowest key), each with a
```

---

[75] **5. Correctness: Pool ranks combine LP values from different states**

`shortwin-keeper::score` · Confidence: 75 · seen in 2/2 runs · NEW

**Description**
An LP changes liquidity after discovery, so the keeper drops eligible pools or gives other pools excessive scores.

**Fix**

```diff
--- a/services/keeper/src/pool_monitor.rs
+++ b/services/keeper/src/pool_monitor.rs
@@ -225,9 +225,9 @@
         })
     }
 
-    /// The accounts [`rank`] reads for it: its WSOL vault and its LP mint.
-    pub fn rank_keys(&self) -> [Pubkey; 2] {
-        [self.sol_vault, self.lp_mint]
+    /// The accounts [`rank`] reads together: pool, WSOL vault and LP mint.
+    pub fn rank_keys(&self) -> [Pubkey; 3] {
+        [self.pool, self.sol_vault, self.lp_mint]
     }
 }
 
@@ -235,12 +235,16 @@
 /// in that order): its locked SOL `⌊λ_bps·R/10⁴⌋` (`R` the vault's balance; M4-SEC-7). 0 for an
 /// implausible pool or accounts that do not read. Unlocked liquidity adds nothing to it.
 pub fn score(c: &CandidatePool, accounts: &[Option<AccountData>]) -> u128 {
-    if !c.plausible {
-        return 0;
-    }
-    let (Some(Some(vault)), Some(Some(lp))) = (accounts.first(), accounts.get(1)) else {
+    let [Some(pool), Some(vault), Some(lp)] = accounts else {
         return 0;
     };
+    let Some(fresh) = CandidatePool::from_account(c.kind, c.pool, pool) else {
+        return 0;
+    };
+    if !fresh.plausible || fresh.rank_keys() != c.rank_keys() {
+        return 0;
+    }
+    let c = &fresh;
     let r = vault
         .data
         .get(64..72)
@@ -688,7 +692,7 @@
 
     /// The next pools to rank within `budget_keys` keys (at least one pool).
     pub fn next_rank_batch(&mut self, budget_keys: usize) -> Vec<CandidatePool> {
-        let n = (budget_keys / 2).max(1).min(self.rank_queue.len());
+        let n = (budget_keys / 3).max(1).min(self.rank_queue.len());
         self.rank_queue.drain(..n).collect()
     }
 
--- a/services/keeper/src/keeper.rs
+++ b/services/keeper/src/keeper.rs
@@ -3239,13 +3239,17 @@
             let t = self.pool_tracks.entry(key).or_default();
             if round == 0 && !t.rank_queue.is_empty() {
                 let batch = t.next_rank_batch(pcfg.rank_keys_per_tick);
-                let keys: Vec<Pubkey> = batch.iter().flat_map(|c| c.rank_keys()).collect();
-                let accs = self.chain.accounts(&keys).await?;
-                let scored: Vec<(u128, pool_monitor::CandidatePool)> = batch
-                    .into_iter()
-                    .zip(accs.chunks(2))
-                    .map(|(c, a)| (pool_monitor::score(&c, a), c))
-                    .collect();
+                let mut scored = Vec::with_capacity(batch.len());
+                // Keep each three-account pool snapshot inside one 100-key RPC call.
+                for group in batch.chunks(33) {
+                    let keys: Vec<Pubkey> = group.iter().flat_map(|c| c.rank_keys()).collect();
+                    let accs = self.chain.accounts(&keys).await?;
+                    scored.extend(
+                        group.iter().cloned()
+                            .zip(accs.chunks(3))
+                            .map(|(c, a)| (pool_monitor::score(&c, a), c)),
+                    );
+                }
                 let t = self.pool_tracks.entry(key).or_default();
                 if t.finish_rank_batch(scored, pcfg.max_candidates) {
                     t.last_eval = None;
--- a/services/keeper/src/config.rs
+++ b/services/keeper/src/config.rs
@@ -614,8 +614,8 @@
         if p.fast_discovery_interval_s > p.discovery_interval_s {
             bail!("pool_select.fast_discovery_interval_s exceeds discovery_interval_s");
         }
-        if p.max_candidates == 0 || p.rank_keys_per_tick < 2 {
-            bail!("pool_select.max_candidates must be positive and rank_keys_per_tick at least 2");
+        if p.max_candidates == 0 || p.rank_keys_per_tick < 3 {
+            bail!("pool_select.max_candidates must be positive and rank_keys_per_tick at least 3");
         }
         if p.discoveries_per_tick == 0 {
             bail!("pool_select.discoveries_per_tick must be positive");
```

---

[75] **6. The last withdrawal leaves carry for the next lender**

`shortwin-math::withdraw_in` · Confidence: 75 · seen in 2/2 runs · NEW

**Description**
A new lender collects old SOL after the last withdrawal preserves both carries; the largest complete second-pass trace transfers 1,000 lamports.

**Fix**

```diff
--- a/crates/shortwin-math/src/pool.rs
+++ b/crates/shortwin-math/src/pool.rs
@@ -1906,6 +1906,9 @@ fn withdraw_in(
     l.shares = sub_u128(l.shares, shares)?;
     p.total_shares = sub_u128(p.total_shares, shares)?;
     if p.total_shares == 0 {
+        p.dust = add_i128(add_i128(p.dust, p.carry)?, to_i128(p.carry_r)?)?;
+        p.carry = 0;
+        p.carry_r = 0;
         // AIR-08: the market has no lender again; SOL that reaches the trader from now on is
         // the protocol's (KI-10), so the next deposit waits for the sweeps, as after a wipe.
         p.wipe_swept = 0;
```

---

[75] **7. Authority changes accept a successor without its signature**

`shortwin::apply_config` · Confidence: 75 · seen in 2/2 runs · NEW

**Description**
An honest authority change can install unusable governance, pause, or coverage keys; lost governance requires a program upgrade to recover.

**Fix (Option A — require governance acceptance; pause and coverage remain unchecked)**

```diff
--- a/programs/shortwin/src/instructions/apply_config.rs
+++ b/programs/shortwin/src/instructions/apply_config.rs
@@ -52,6 +52,8 @@
         bump = oracle_cache.bump
     )]
     pub oracle_cache: Option<Box<Account<'info, OracleCache>>>,
+    /// The successor must accept a change of governance.
+    pub new_governance: Option<Signer<'info>>,
 }
 
 pub fn handler(ctx: Context<ApplyConfig>) -> Result<()> {
@@ -76,6 +78,16 @@
         );
         let change = c.pending.change()?;
         c.check_change(&change)?;
+        if let ConfigChange::Authorities { governance, .. } = &change {
+            if *governance != c.governance {
+                let successor = ctx
+                    .accounts
+                    .new_governance
+                    .as_ref()
+                    .ok_or_else(|| error!(ShortwinError::Unauthorized))?;
+                require_keys_eq!(successor.key(), *governance, ShortwinError::Unauthorized);
+            }
+        }
         let applied = (c.pending.kind, c.pending.hash());
         c.apply(&change, clock.unix_timestamp)?;
         c.pending.clear();
```

**Fix (Option B — require every changed enabled authority to sign)**

```diff
--- a/programs/shortwin/src/instructions/apply_config.rs
+++ b/programs/shortwin/src/instructions/apply_config.rs
@@ -76,6 +76,22 @@
         );
         let change = c.pending.change()?;
         c.check_change(&change)?;
+        if let ConfigChange::Authorities {
+            governance, pause_authority, coverage_authority, ..
+        } = &change {
+            for (old, new) in [
+                (c.governance, *governance),
+                (c.pause_authority, *pause_authority),
+                (c.coverage_authority, *coverage_authority),
+            ] {
+                if new != old && new != Pubkey::default() {
+                    require!(
+                        ctx.remaining_accounts.iter().any(|a| a.key() == new && a.is_signer),
+                        ShortwinError::Unauthorized
+                    );
+                }
+            }
+        }
         let applied = (c.pending.kind, c.pending.hash());
         c.apply(&change, clock.unix_timestamp)?;
         c.pending.clear();
```

---

[75] **8. Hashless venue guards let outsiders block new opens**

`shortwin::flag_venue_upgraded` · Confidence: 75 · seen in 2/2 runs · NEW

**Description**
An outsider can extend a growable venue without a stored ELF fingerprint, then block new opens until Emergency revalidation.

**Fix (Option A — Require hashes before enabling fingerprint-capable venues; existing guards need bootstrap)**

```diff
--- a/programs/shortwin/src/instructions/initialize_config.rs
+++ b/programs/shortwin/src/instructions/initialize_config.rs
@@ -156,7 +156,8 @@
         let mut g = ctx.accounts.venue_guard.load_init()?;
         g.version = VENUE_GUARD_VERSION;
         g.bump = ctx.bumps.venue_guard;
-        g.upgraded = 0;
+        g.upgraded = ((1u32 << crate::state::GUARD_ELF_VENUES) - 1)
+            << crate::state::GUARD_ELF_FIRST;
         g.guard_slot = args.guard_slot;
     }
     let extras = [
--- a/programs/shortwin/src/instructions/revalidate_venue.rs
+++ b/programs/shortwin/src/instructions/revalidate_venue.rs
@@ -72,6 +72,7 @@
     let slot = read_deploy_slot(&ctx.accounts.program_data)?;
     let hashed = expected_elf_hash != [0u8; 32];
     let fingerprinted = VenueGuard::elf_index(venue).is_some();
+    require!(!fingerprinted || hashed, ShortwinError::InvalidParams);
     if slot != expected_slot {
         // AIR-01: the reviewed deploy, only extended since, still holds the reviewed ELF.
         require!(hashed && fingerprinted, ShortwinError::VenueSlotMismatch);
```

**Fix (Option B — Require and verify hashes; repeats hashing after a slot change and needs bootstrap)**

```diff
--- a/programs/shortwin/src/instructions/initialize_config.rs
+++ b/programs/shortwin/src/instructions/initialize_config.rs
@@ -157,6 +157,11 @@
         g.version = VENUE_GUARD_VERSION;
         g.bump = ctx.bumps.venue_guard;
         g.upgraded = 0;
+        for venue in 0..crate::constants::VENUE_PROGRAMS as u8 {
+            if VenueGuard::elf_index(venue).is_some() {
+                g.upgraded |= 1u32 << venue;
+            }
+        }
         g.guard_slot = args.guard_slot;
     }
     let extras = [
--- a/programs/shortwin/src/instructions/revalidate_venue.rs
+++ b/programs/shortwin/src/instructions/revalidate_venue.rs
@@ -72,6 +72,13 @@
     let slot = read_deploy_slot(&ctx.accounts.program_data)?;
     let hashed = expected_elf_hash != [0u8; 32];
     let fingerprinted = VenueGuard::elf_index(venue).is_some();
+    require!(!fingerprinted || hashed, ShortwinError::InvalidParams);
+    if fingerprinted {
+        require!(
+            trimmed_elf_hash(&ctx.accounts.program_data)? == Some(expected_elf_hash),
+            ShortwinError::VenueSlotMismatch
+        );
+    }
     if slot != expected_slot {
         // AIR-01: the reviewed deploy, only extended since, still holds the reviewed ELF.
         require!(hashed && fingerprinted, ShortwinError::VenueSlotMismatch);
```

---

Findings List

| # | Confidence | Title |
|---|---|---|
| 1 | [75] | Correctness: The keeper checks the proposal floor at the wrong price |
| 2 | [75] | Correctness: The keeper skips pools after a floor failure |
| 3 | [75] | Correctness: The keeper omits supported coin creator extras |
| 4 | [75] | Correctness: Pool ranks round positive locked SOL to zero |
| 5 | [75] | Correctness: Pool ranks combine LP values from different states |
| 6 | [75] | The last withdrawal leaves carry for the next lender |
| 7 | [75] | Authority changes accept a successor without its signature |
| 8 | [75] | Hashless venue guards let outsiders block new opens |

---

## Leads

_Vulnerability trails with concrete code smells where the full exploit path could not be completed in one analysis pass. These are not false positives — they are high-signal leads for manual review. Not scored._

- **The ALT parser accepts a truncated metadata header** — `shortwin-common::parse_lookup_table` · seen in 1/2 runs · NEW — Code smells: a missing metadata slice becomes an empty list; the full header length is unchecked — The parser accepts truncated metadata; a future untrusted loader could conceal malformed tables as empty tables.
- **Coverage preserves an earlier liquidity low** — `shortwin-keeper::check_foreign` · seen in 1/2 runs · NEW — Code smells: earlier liquidity lows persist after foreign observations; settlement effects remain unverified — An LP can preserve an earlier liquidity low, but this pass does not prove a resulting settlement change or profit.
- **The keeper reads layouts without their version checks** — `shortwin-keeper::decode_zc` · seen in 2/2 runs · NEW — Code smells: The decoder checks the owner, discriminator and byte length, but it omits supported-version checks. — The keeper accepts layouts without version checks; a future upgrade can make its decoded fields incompatible.
- **A failed RPC read removes pending rank candidates** — `shortwin-keeper::next_rank_batch` · seen in 1/2 runs · NEW — Code smells: The helper drains the rank queue before the RPC read; the caller returns on error without restoring the removed candidates. — The keeper loses a pending batch after an RPC error until rediscovery. No outsider trigger or stranded funds are proved.
- **The keeper retains readiness after external accounts close** — `shortwin-keeper::prepare_pamm_trader` · seen in 1/2 runs · NEW — Code smells: cached readiness skips account reads; recipient ATA closure and repair remain unverified — The keeper trusts cached readiness after accounts close, but the recipient’s ability to close the required ATA remains unverified.
- **Normal curve quotes use the original mint supply** — `shortwin-math::pump_curve` · seen in 2/2 runs · NEW — Code smells: pump_curve uses PUMP_CURVE_MCAP_SUPPLY for ordinary fees; only the separate synthetic resolver receives the live supply. — Normal curve quotes can choose a different fee after a burn under multiple tiers. The documented single-tier schedule prevents this today.
- **The base mint reader omits account type checks** — `shortwin-venue-state::read_base` · seen in 2/2 runs · NEW — Code smells: read_base accepts any length of at least 82 bytes under a token owner and omits mint padding and account type checks. — A future caller without a trusted mint key can read token-account bytes as mint supply. Current caller keys block that substitution.
- **Wrapped observations can preserve a low liquidity estimate** — `shortwin::assess` · seen in 1/2 runs · NEW — Code smells: the liquidation floor trusts a prior EMA; repeated withdrawals can freeze its history — An LP can preserve a low liquidity history; the current observation sequence and forced-liquidation payoff remain unverified.
- **Exit instructions omit the position status check** — `shortwin::close_short` · seen in 2/2 runs · NEW — Code smells: close_short, liquidate, expire, sol_settle omit status guards — A future instruction that persists a non-open status needs matching exit checks; current writers only create open positions.
- **Small donations can extend a settlement hold** — `shortwin::deposit` · seen in 2/2 runs · NEW — Code smells: the shared vest gates deposits; public sweeps renew the vest — Small donations can renew an active settlement hold; sustained blockage and a fix that preserves historical rewards remain unverified.
- **Pool vault loads omit token field constraints.** — `shortwin::deposit` · seen in 2/2 runs · NEW — Code smells: The canonical vault PDA and stored key are checked, but deposit, withdraw, open_short, close_short, liquidate, and expire omit explicit token field constraints. — A future vault importer can admit the wrong asset because the loaders omit checks of the vault mint, authority, and token program.
- **The guard keeps twelve bytes of the ELF hash** — `shortwin::elf_fingerprint_of` · seen in 2/2 runs · NEW — Code smells: elf_fingerprint_of stores only the first twelve SHA-256 bytes; is_upgrade compares that prefix after the account length changes. — The guard compares twelve hash bytes after ProgramData grows. No practical pair of valid executable files with the same prefix is proved.
- **A supply burn can raise the PumpSwap risk fee.** — `shortwin::exit_risk_cap_bps` · seen in 1/2 runs · NEW — Code smells: PumpSwap resolves both live and EMA fee tiers from the current mint supply, so the EMA price does not fix this input. — A holder can burn tokens before liquidation and raise the risk fee; this pass does not prove a profitable liquidation.
- **Existing Pump UVAs skip full validation at listing** — `shortwin::init_uva` · seen in 2/2 runs · NEW — Code smells: Both init_uva helpers return for a nonempty venue-owned UVA without decoding its discriminator, layout or user. — Listing accepts existing UVAs without decoding their user. A future venue layout can pass listing but fail trading; no current wrong-user path is proved.
- **Temporary withdrawals change live settlement inputs** — `shortwin::inputs` · seen in 2/2 runs · NEW — Code smells: live reserves feed S1/S2; only the CR leg uses the EMA floor — Temporary LP withdrawals alter live settlement legs, but a complete production sequence and loss beyond the documented AIR-20 residual remain unproved.
- **The adapters copy every fee tier into the heap.** — `shortwin::math_tiers` · seen in 1/2 runs · NEW — Code smells: Both math_tiers functions collect every parsed tier into a Vec, while tiers_at limits the count only by account length. — A fee-config expansion can exhaust the program heap before a reader returns no route; this pass does not prove a failing tier count.
- **The Raydium readers omit vault authority checks** — `shortwin::parse` · seen in 2/2 runs · NEW — Code smells: Both live parsers check vault keys, token programs and mints but omit the authority comparison that their adoption verifiers require. — The live readers omit vault authority checks. A future venue-authorized change can leave accepted reserves unusable; no current public authority-change path is proved.
- **The curve adapter uses a fixed supply for fees.** — `shortwin::pricing_inputs` · seen in 1/2 runs · NEW — Code smells: The adapter uses fixed supply for ordinary and EMA fees, live supply for synthetic fees, and a parser that accepts several tiers. — The curve adapter selects the wrong fee after burns if Pump adds tiers; this review does not verify such a current schedule.
- **A burn can raise the curve synthetic risk fee.** — `shortwin::pricing_inputs` · seen in 1/2 runs · NEW — Code smells: pricing_inputs resolves synthetic_fees from the current supply, and risk quotes retain it under the position hard cap. — A holder can raise the synthetic risk fee after a burn under multiple tiers; this pass does not prove a liquidation.
- **The curve fee gates use a different supply basis.** — `shortwin::pricing_inputs` · seen in 1/2 runs · NEW — Code smells: Ordinary fee gates use the fixed 10^15 supply, while synthetic exits resolve their fee from the live supply. — The curve fee gates can admit a lower fee than a synthetic exit pays; this requires a tier boundary between the supply bases.
- **Curve risk quotes retain the fee resolved at spot** — `shortwin::read_state_boxed_for_exit` · seen in 2/2 runs · NEW — Code smells: Exit reads request an EMA fee only for PumpSwap, while ordinary curve fees come from live reserves. — Curve exits retain the live fee in historical risk quotes. No supported tier change and resulting wrong exit decision is proved.
- **Cashback cleanup depends on external claim instructions** — `shortwin::sweep_trader` · seen in 2/2 runs · NEW — Code smells: sweep_trader leaves the reward bit clear while external cashback remains; it reads the cashback accounts but cannot empty them. — Unclaimable external cashback can keep deposits blocked after a wipe or the last withdrawal. No current failure of the external claim is proved.
- **Repeated sync calls discard fractional lender credits** — `shortwin::sync_lender` · seen in 2/2 runs · NEW — Code smells: per-touch floor; snapshots advance without a lender remainder — Unsigned syncs discard fractional lender credits; a complete two-release trace loses one lamport, while material aggregate harm remains unproved.

---

> ⚠️ This review was performed by an AI assistant. AI analysis can never verify the complete absence of vulnerabilities and no guarantee of security is given. Team security reviews, bug bounty programs, and on-chain monitoring are strongly recommended.
