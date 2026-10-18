FINDING | program: shortwin-keeper | function: evaluate | bug_class: wrong-window-floor-reference | group_key: shortwin-keeper | evaluate | wrong-window-floor-reference
file: services/keeper/src/pool_monitor.rs:376
title: The keeper accepts a pool that the proposal floor rejects
seam: economics×asymmetry
actor: A permissionless pool creator or trader who supplies a deep pool with a lower spot price.
root_cause: The keeper evaluates the proposal floor at P_ema, but propose_pool evaluates it at min(P_ema, P_cand).
path: A valid low-spot candidate enters discovery → evaluate records window_floor=true at P_ema → plan selects its greater D → propose_pool rejects PoolTooThin → the keeper backs off and selects it again.
proof: All amounts below are base units; 1 SOL = 1e9 lamports. Use pool_min_real_sol=25e9, lambda_min=5000, P_ema=1e18, an armed failed incumbent, and two executable 25-bps pools with fully burned LP. A has X=500e9, Y=5e9, D=50e9 and P_cand=1e16. B has X=Y=D=30e9 and P_cand=1e18. Both satisfy verify_candidate's P_ema eligibility floor. evaluate calls window_floor_ok(&a, params, p_ref), so it records locked SOL of 50e9 for A and 30e9 for B. plan ranks by D and selects A. propose_pool.rs:315-318 instead passes window_price(p_ref,&adoption); A has 50e9*isqrt(1e16)/1e9=5e9, below 25e9, while B has 30e9 and passes. Take one carried position q=150000000, L_slow=30e9, min_open_fee_ppm=2500, the default caps (fee_size_cap_x100=300, fee_oi_cap_x100=750), and an empty oracle cache (Tier C). Both candidates have X0>=30e9 at both checked prices, so the fee-size cap is at least 225000000 and the OI cap is at least 562500000; Tier C impact caps are looser. The position fits both; the false floor alone rejects A. keeper.rs:3532-3587 only records backoff and alerts after a refusal; it does not exclude A. This is a static correctness trace, not a claim that arbitrage leaves differing pool prices unchanged indefinitely.
guards: Candidate accounts remain real, owned by the pinned Raydium program, and reverified on chain; those checks reject the keeper's selected action but do not choose B.
economic_delta: The keeper fails to start a feasible collection window, so existing positions remain on the failed incumbent and their rent and settlement clocks continue.
description: The keeper selects a pool that the on-chain floor rejects, so borrowers miss a viable pool switch.
fix: Use the proposal's minimum price when the keeper computes a candidate's window floor.
minimal_diff:
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

FINDING | program: shortwin-keeper | function: plan | bug_class: false-window-floor-monotonicity | group_key: shortwin-keeper | plan | false-window-floor-monotonicity
file: services/keeper/src/pool_monitor.rs:538
title: The keeper stops before it checks another viable pool
seam: economics×asymmetry
actor: A pool creator or trader who makes the deepest candidate fail the proposal floor.
root_cause: plan treats a failed floor on the greatest D as proof that every other candidate fails.
path: Correct proposal-floor verdicts reach plan → A ranks first by D but has window_floor=false → the early PoolTooThin branch returns no action → shallower B is never selected.
proof: This path works after shortwin-keeper|evaluate|wrong-window-floor-reference is fixed. With the same A and B above, corrected evaluation yields A(D=50e9,floor=false,fits=true), B(D=30e9,floor=true,fits=true). For an armed failed incumbent with D_inc=50e9 and fast_min_bps=2500, decide accepts OpenWindow for both. At pool_monitor.rs:534-540 the first candidate's false floor produces PoolTooThin, although B passes. The later fallback search is entered only when the first candidate fails carried_fit. The proposal floor depends on each candidate's own price and is not monotone in D. This is a separate masked correctness defect; fixing evaluate alone leaves the keeper unable to propose B.
guards: On-chain floor validation prevents a thin adoption; it cannot make the keeper examine the remaining candidates.
economic_delta: A valid route stays unused while existing positions remain on the failed pool.
description: The keeper skips a viable pool after a deeper pool fails the floor.
fix: Search the remaining candidates when the deepest candidate fails either the floor or the carried-position caps.
minimal_diff:
```diff
--- a/services/keeper/src/pool_monitor.rs
+++ b/services/keeper/src/pool_monitor.rs
@@ -531,18 +531,14 @@
         return out;
     };
     out.best = Some((c.pool, d));
-    // M4V-1: a window's candidate also needs the window floor, which is monotone in `D` at the
-    // market's one `P_ema`: if the deepest candidate misses it, every candidate does.
+    // The proposal floor also uses each candidate's spot price, so it is not monotone in D.
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
+        Ok(Proposal::OpenWindow | Proposal::ImproveWindow) if !window_floor || !c.fits() => {
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

FINDING | program: shortwin-keeper | function: score | bug_class: rounded-lock-share-ranking | group_key: shortwin-keeper | score | rounded-lock-share-ranking
file: services/keeper/src/pool_monitor.rs:252
title: The keeper drops valid pools with small locked LP shares
seam: access×economics
actor: An ordinary LP who adds unlocked liquidity, or an honest pool with a small locked share.
root_cause: score floors the locked LP fraction to whole basis points before it values locked SOL.
path: An LP adds proportional unlocked liquidity → true locked depth remains unchanged → lambda_bps rounds to zero → score returns zero → rank removes the still-eligible candidate.
proof: Let the current pool counter C=250000000000001, LP mint supply S=249975000000001, and X=Y=C. The locked LP amount C-S=25000000000. The program's raw-ratio locked_depth is (C-S)*isqrt(X*Y)/C=25000000000. At P_ema=P_cand=1e18 this is 25 SOL, so the 25SOL window floor passes. The keeper computes lambda_bps=floor((C-S)*10000/C)=0 and score=0. rank at pool_monitor.rs:258 retains only scores greater than zero. The same endpoint can be reached by proportional unlocked deposits without a fall in D; the depositor can later withdraw the added liquidity. This arithmetic uses the same current counter and supply, so the separate stale-counter defect is unnecessary.
guards: verify_candidate uses the raw ratio in crates/shortwin-math/src/pool_select.rs:41-55, but ranking removes the pool before evaluate can call it.
economic_delta: A pool with 25 SOL of permanent locked depth receives zero ranking value, and its liquidity cannot serve the keeper's pool switch.
description: An LP can hide an eligible pool from the keeper when unlocked deposits reduce its locked share below one basis point.
fix: Compute the ranking score from the raw locked LP fraction.
minimal_diff:
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
 
@@ -249,7 +249,11 @@
     let Ok(m) = mint::read_base(&lp.owner.to_bytes(), &lp.data) else {
         return 0;
     };
-    u128::from(lambda_bps(c.lp_counter, m.supply)) * u128::from(r) / 10_000
+    let locked = c.lp_counter.saturating_sub(m.supply);
+    if c.lp_counter == 0 {
+        return 0;
+    }
+    u128::from(locked) * u128::from(r) / u128::from(c.lp_counter)
 }
 
 /// The best `k` candidates by score (highest first; among equals the lowest key), each with a
```

FINDING | program: shortwin-keeper | function: score | bug_class: mixed-snapshot-lp-ranking | group_key: shortwin-keeper | score | mixed-snapshot-lp-ranking
file: services/keeper/src/pool_monitor.rs:252
title: A deposit hides a valid pool through mismatched LP snapshots
seam: access×economics
actor: Any LP who deposits after discovery and before the keeper reads the ranking accounts.
root_cause: score subtracts a newly fetched LP supply from the pool counter cached at discovery.
path: discover caches the pool's old lp_counter → an LP deposits unlocked liquidity → ranking fetches the higher LP supply and vault balance → the old counter is below the new supply → score returns zero and rank drops the pool.
proof: At discovery, set counter=50000000000, supply=25000000000, X=Y=50000000000. Locked depth is25e9. A proportional deposit adds25000000001 tokens and25000000001 lamports and mints25000000001 LP. The new counter is75000000001, supply50000000001, and X=Y=75000000001; true D remains25e9 exactly. CandidatePool.lp_counter remains50e9. score calls lambda_bps(50e9,50000000001), whose checked_sub fails and returns0. The attacker commits about 25.000000001 SOL plus tokens temporarily and can withdraw after ranking. CandidatePool::from_account stores lp_counter at pool_monitor.rs:223; rank_keys at:229 fetches only the vault and LP mint. keeper.rs:3240-3253 performs this later read, and next_rank_batch can spread discovery across ticks. The same race exists with the raw-ratio rounding fix: stale_counter.saturating_sub(current_supply) remains zero.
guards: The candidate's on-chain depth and eligibility stay valid; the keeper discards it before its verifier sees it. A third party may still call propose_pool directly.
economic_delta: A recoverable liquidity deposit removes a pool with 25 SOL of locked depth from the keeper's candidates until a new ranking succeeds.
description: An LP can hide a valid pool from the keeper with a deposit between discovery and ranking.
fix: Read each pool counter, vault, and LP mint from one RPC snapshot before the keeper ranks that pool.
minimal_diff:
```diff
--- a/services/keeper/src/pool_monitor.rs
+++ b/services/keeper/src/pool_monitor.rs
@@ -226,8 +226,8 @@
     }
 
     /// The accounts [`rank`] reads for it: its WSOL vault and its LP mint.
-    pub fn rank_keys(&self) -> [Pubkey; 2] {
-        [self.sol_vault, self.lp_mint]
+    pub fn rank_keys(&self) -> [Pubkey; 3] {
+        [self.pool, self.sol_vault, self.lp_mint]
     }
 }
 
@@ -238,9 +238,17 @@
     if !c.plausible {
         return 0;
     }
-    let (Some(Some(vault)), Some(Some(lp))) = (accounts.first(), accounts.get(1)) else {
+    let (Some(Some(pool)), Some(Some(vault)), Some(Some(lp))) =
+        (accounts.first(), accounts.get(1), accounts.get(2))
+    else {
         return 0;
     };
+    let Some(fresh) = CandidatePool::from_account(c.kind, c.pool, pool) else {
+        return 0;
+    };
+    if !fresh.plausible || fresh.sol_vault != c.sol_vault || fresh.lp_mint != c.lp_mint {
+        return 0;
+    }
     let r = vault
         .data
         .get(64..72)
@@ -249,7 +257,7 @@
     let Ok(m) = mint::read_base(&lp.owner.to_bytes(), &lp.data) else {
         return 0;
     };
-    u128::from(lambda_bps(c.lp_counter, m.supply)) * u128::from(r) / 10_000
+    u128::from(lambda_bps(fresh.lp_counter, m.supply)) * u128::from(r) / 10_000
 }
 
 /// The best `k` candidates by score (highest first; among equals the lowest key), each with a
@@ -688,7 +696,7 @@
 
     /// The next pools to rank within `budget_keys` keys (at least one pool).
     pub fn next_rank_batch(&mut self, budget_keys: usize) -> Vec<CandidatePool> {
-        let n = (budget_keys / 2).max(1).min(self.rank_queue.len());
+        let n = (budget_keys / 3).max(1).min(self.rank_queue.len());
         self.rank_queue.drain(..n).collect()
     }
 
--- a/services/keeper/src/keeper.rs
+++ b/services/keeper/src/keeper.rs
@@ -3239,13 +3239,15 @@
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
+                // Keep each pool, vault and LP mint in one getMultipleAccounts snapshot.
+                for group in batch.chunks(33) {
+                    let keys: Vec<Pubkey> = group.iter().flat_map(|c| c.rank_keys()).collect();
+                    let accs = self.chain.accounts(&keys).await?;
+                    scored.extend(group.iter().cloned().zip(accs.chunks(3)).map(|(c, a)| {
+                        (pool_monitor::score(&c, a), c)
+                    }));
+                }
                 let t = self.pool_tracks.entry(key).or_default();
                 if t.finish_rank_batch(scored, pcfg.max_candidates) {
                     t.last_eval = None;
```
fix_scope: The second hunk is the required caller adjustment in out-of-scope services/keeper/src/keeper.rs. Each group has at most 99 keys, below RpcChain's 100-key chunk boundary. Integrate this fix with the raw-fraction fix; each diff above starts from the frozen source.

FINDING | program: shortwin | function: flag_venue_upgraded | bug_class: unhashed-slot-upgrade-latch | group_key: shortwin | flag_venue_upgraded | unhashed-slot-upgrade-latch
file: programs/shortwin/src/instructions/flag_venue_upgraded.rs:126
title: An extension can block opens when the guard lacks a fingerprint
seam: access×asymmetry
actor: Any fee payer after initialization or an honest revalidation without an ELF hash.
root_cause: The program enables an unflagged venue with no fingerprint and then treats any changed deploy slot as an upgrade.
path: initialize_config sets upgraded=0 and no fingerprints, or revalidate_venue accepts a zero hash and clears one → an outsider extends a growable venue ProgramData → flag_venue_upgraded sees a different slot and sets its bit → opens and that venue's proposals fail until Emergency revalidation.
proof: initialize_config.rs:156-160 assigns upgraded=0 and guard_slot but no guard_len or guard_elf. VenueGuard::elf_fingerprint returns None when guard_len=0. revalidate_venue.rs:72-83 accepts expected_elf_hash=[0;32] when expected_slot matches; lines104-108 then clear the fingerprint and latch. flag_venue_upgraded.rs:128-136 skips is_upgrade if that fingerprint is absent and sets the bit on slot inequality. In the locally installed solana-bpf-loader-program 4.2.2 source, lib.rs:793-794 calls common_extend_program(...,false); :926-937 checks the authority only if check_authority; :973-996 grows the account and sets slot=clock_slot. An outsider funds a legal extension (10 KiB works with the optional minimum-size feature), without the venue's upgrade signer. Raydium bits 3 and 4 share GUARD_MASK_RAYDIUM, so either extension blocks opens on both Raydium kinds. This state is explicitly documented in the source and remains reachable; acknowledgement does not add a guard. Current program data lengths are not queried here; the trace uses the growable venues that this source itself identifies.
guards: Fixed ProgramData keys, loader ownership, and tag3 prevent account substitution. Fingerprinted venues detect an unchanged ELF correctly. Exits do not read this latch, so this finding does not claim existing positions become permanently locked.
economic_delta: The outsider obtains a veto over new Raydium opens and one venue's candidate adoption for the cost of extension rent and transaction fees.
description: An outsider can block new opens with a program extension while the venue guard has no ELF fingerprint.
fix: Keep growable venues latched until revalidation supplies an ELF hash, and reject later hashless revalidation.
minimal_diff:
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

LEAD | program: shortwin | function: elf_fingerprint_of | bug_class: truncated-upgrade-fingerprint | group_key: shortwin | elf_fingerprint_of | truncated-upgrade-fingerprint
file: programs/shortwin/src/state/venue_guard.rs:52
title: The guard compares only twelve bytes of the reviewed hash
seam: access×economics
actor: A venue upgrade authority that can choose both the reviewed and later ELF files.
code_smells: The guard stores a 96-bit fingerprint, and a grown ProgramData account passes the unchanged-ELF test when those 96 bits match.
trace: VenueGuard::GUARD_ELF_LEN=12 → revalidate_venue stores elf_fingerprint_of(expected_elf_hash) → flag_venue_upgraded::is_upgrade compares only those 12 bytes after an extension. A malicious venue authority could search an ELF pair with one prefix, obtain review of the first, then extend and upgrade to the second. The generic chosen-pair collision work is about 2^48 hashes; this audit supplies no collision or practical budget. The source's AIR2-07 comment already identifies the reduced bound.
description: A venue authority can seek two ELF files with one fingerprint; this review does not prove a practical collision.

LEAD | program: shortwin-keeper | function: decode_zc | bug_class: hardening-version-unchecked | group_key: shortwin-keeper | decode_zc | hardening-version-unchecked
file: services/keeper/src/view.rs:48
title: The keeper reads account layouts without checking their versions
seam: access×economics
actor: An honest protocol upgrade that retains an account discriminator and size but changes its layout version.
code_smells: decode_zc checks the owner and discriminator, but load_with_positions accepts GlobalConfig, Market, and VenueExtras without their version guards.
trace: view.rs:48-56 copies sizeof(T) bytes through pod_read_unaligned; load_with_positions uses the copies to choose venues and evaluate economic rules. On-chain accounts enforce CONFIG_VERSION or MARKET_VERSION on these paths. A later layout version with compatible size can therefore produce incorrect keeper decisions while transactions reject. No same-version attacker write or present fund loss was proved.
description: A future layout upgrade can make the keeper use incompatible account fields because it does not check the stored versions.
