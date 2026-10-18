FINDING | program: shortwin-keeper | function: score | bug_class: stale-lp-counter | group_key: shortwin-keeper | score | stale-lp-counter
location: services/keeper/src/pool_monitor.rs:237; CandidatePool::rank_keys at :229; next_rank_batch at :690; services/keeper/src/keeper.rs:3241-3250; services/common/src/rpc.rs:261-289.
title: The keeper combines LP values from different states
assumption: A pool's internal LP counter stays constant between discovery and ranking.
violation: An ordinary proportional deposit or withdrawal changes the internal counter before the separate vault and LP-mint read.
path: discover → CandidatePool::from_account saves lp_counter → unlocked LP deposits → rank reads only vault and LP mint → score uses old counter → rank drops the pool.
proof: Discovery records counter=1,000,000,000, circulating supply=500,000,000 and SOL reserve=100,000,000,000. A proportional deposit doubles total liquidity: counter=2,000,000,000, supply=1,500,000,000, reserve=200,000,000,000. Locked SOL remains (2,000,000,000−1,500,000,000)*200,000,000,000/2,000,000,000=50,000,000,000. score instead evaluates lambda_bps(1,000,000,000,1,500,000,000)=0. rank retains only positive scores at pool_monitor.rs:258. Thus a valid candidate disappears before its fresh on-chain verifier runs. A withdrawal causes the converse overstatement. This does not require a counterfeit account or malicious RPC.
guards: from_account parses the venue owner/layout and plausible status; later evaluate uses verify_candidate and transactions are simulated. These prevent invalid adoption, but cannot recover a candidate discarded before evaluation.
impact: An ordinary LP operation can remove an eligible alternate pool until the next discovery, or fill the bounded candidate set with exaggerated scores. No direct on-chain theft is established.
description: An LP can change pool liquidity after discovery, so the keeper discards eligible pools or gives other pools excessive scores.
fix: Read each pool, SOL vault and LP mint in one RPC batch before the keeper calculates its score.
fix_notes: The RPC backend chunks at 100 keys. Three-key groups must stay together: candidate 34 starts at index 99 and otherwise spans two snapshots. The diff keeps each request at 99 keys. It preserves the existing bps formula for this isolated fix; apply the separate rounded-lock-share-ranking fix too. Diffs are unapplied and uncompiled.
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

FINDING | program: shortwin-keeper | function: score | bug_class: rounded-lock-share-ranking | group_key: shortwin-keeper | score | rounded-lock-share-ranking
location: services/keeper/src/pool_monitor.rs:253-260; crates/shortwin-math/src/pool_select.rs:19-58.
title: The keeper rounds positive locked SOL to zero
assumption: Multiplying a whole-bps locked share by the reserve preserves the exact locked SOL score.
violation: A positive locked share below one basis point rounds to zero before multiplication.
path: proportional unlocked liquidity dilutes the locked share → score floors lambda to whole bps → rank removes the candidate → evaluate never checks its positive locked depth.
proof: Use one coherent snapshot with counter=1,000,000,000, supply=999,950,000 and SOL reserve=1,000,000,000,000,000. The raw locked SOL equals 50,000*1,000,000,000,000,000/1,000,000,000=50,000,000,000 (50 SOL). lambda_bps floors 0.5 bps to zero, so score returns zero. With X=Y=1,000,000,000,000,000, locked_depth instead returns 50,000,000,000; at P_ref=10^18 the window floor is 50 SOL and exceeds the 25-SOL default. The example is capital intensive but demonstrates a deterministic correctness defect without inconsistent snapshots.
guards: Genuine pool metadata and fresh LP/vault accounts do not prevent this discrepancy. The on-chain locked_depth helper deliberately uses the raw ratio and documents proportional-deposit invariance. rank removes zero scores before verification.
impact: The keeper omits positive-depth, eligible pools and can misorder other pools. The program's eligibility checks remain intact.
description: An LP can dilute the locked share below one basis point, so the keeper drops a pool that still holds positive locked SOL.
fix: Multiply the locked LP amount by the SOL reserve before the keeper divides by the total LP counter.
fix_notes: This independent diff targets the frozen source; after the stale-lp-counter fix, c must refer to the freshly parsed pool. The product fits u128 because both factors are u64.
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

FINDING | program: shortwin-keeper | function: evaluate | bug_class: wrong-window-floor-reference | group_key: shortwin-keeper | evaluate | wrong-window-floor-reference
location: services/keeper/src/pool_monitor.rs:361-381; programs/shortwin/src/instructions/propose_pool.rs:314-321; programs/shortwin/src/engine/pool_select.rs:363-380.
title: The keeper checks the proposal floor at the wrong price
assumption: A proposal checks its collection-window floor at the market EMA price alone.
violation: propose_pool actually calls window_price and uses min(P_ema,P_candidate).
path: incumbent halts at a high EMA → lower-priced alternate pool passes the keeper's floor → plan proposes it → simulation rejects PoolTooThin repeatedly.
proof: Let P_ema=10^18 and the candidate be fully locked with X=400,000,000,000 and Y=16,000,000,000. Then D=80,000,000,000 and P_candidate=4*10^16. With pool_min_real_sol=25,000,000,000, the keeper's window_floor_ok at P_ema gives 80 SOL and true; the on-chain call at window_price gives 16 SOL and false. General eligibility still passes: at lambda_min=5,000 the eligibility reserve is 160 SOL. Take pool_d=100,000,000,000, open_positions=0, a failed incumbent with an armed fast path and no current window. D=80 billion misses the normal 25% dominance requirement (125 billion) but meets fast_min_bps=2,500 (25 billion), so decide returns OpenWindow. carried_fit returns Fits immediately with zero positions. A genuine owner/layout-verified candidate reaches the discrepancy.
guards: verify_candidate uses the correct general eligibility price; it does not apply the proposal-only floor. Simulation and the on-chain require prevent unsafe adoption, but they do not choose another candidate.
impact: The keeper repeatedly attempts a refused proposal and can delay a usable alternate pool.
description: The keeper accepts a candidate below the proposal floor because it checks the market EMA instead of the lower candidate price.
fix: Pass window_price to the keeper's proposal floor check, as propose_pool does.
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
location: services/keeper/src/pool_monitor.rs:539-573; programs/shortwin/src/engine/pool_select.rs:375-380.
title: The keeper skips shallower pools after a floor failure
assumption: Window-floor success is monotone in depth because every candidate uses one shared price.
violation: Correct proposal evaluation uses each candidate's min(P_ema,P_candidate).
path: evaluate produces proposal-correct floors → deepest pool fails its own floor → plan stops at PoolTooThin → a shallower candidate with sufficient locked SOL never gets proposed.
proof: This path is masked by shortwin-keeper|evaluate|wrong-window-floor-reference and becomes reachable after that fix. Use the previous finding's armed situation: P_ema=10^18, incumbent depth=100 billion, no positions and no window. Candidate A has X=400 billion,Y=16 billion, D=80 billion and a false 16-SOL window floor. Candidate B has X=Y=40 billion, D=40 billion and a true 40-SOL floor. Both are fully locked, eligible, fit trivially and qualify for OpenWindow; neither satisfies normal dominance. A sorts first. The first match arm sets PoolTooThin and never checks B. The next arm searches other candidates only for !c.fits(), which A does not satisfy. No candidate ordering or tie is ambiguous.
guards: The program's floor protects every proposal. Today evaluate's different defect makes this example try A instead of suppressing both; repairing evaluate alone exposes this separate search failure.
impact: The keeper can leave a failed incumbent in place despite an alternate pool that the program accepts.
description: The keeper stops after the deepest pool fails its floor, so it skips a shallower pool that the program accepts.
fix: Search all ranked candidates for a window proposal that passes both the floor and the carried-position check.
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

FINDING | program: shortwin-keeper | function: resolve_entry | bug_class: unsupported-coin-creator-placeholder | group_key: shortwin-keeper | resolve_entry | unsupported-coin-creator-placeholder
location: services/keeper/src/venue.rs:657-687, 704, 737-746; programs/shortwin/src/venues/mod.rs:124-138,1260-1271; programs/shortwin/src/state/venue_extras.rs:60.
title: The keeper omits supported coin creator extras
assumption: The market cache cannot resolve the coin_creator placeholder.
violation: The program resolves PumpSwap coin_creator from pump_swap_cache::CACHED_COIN_CREATOR, while the keeper returns None for that placeholder.
path: authorized extras set contains a PumpSwap coin_creator PDA template → resolve_entry returns None → choice_of drops the complete set → extras_candidates offers only other sets or none.
proof: Take a PumpSwap market whose CACHED_COIN_CREATOR equals key C. Configure an otherwise valid template with one SEED_PLACEHOLDER whose len=PLACEHOLDER_COIN_CREATOR (4), zero seed bytes, and a permitted PDA program E. The program's Placeholders::key returns C and parse derives PDA(E,[C]). resolve_entry matches only IDs 0 through 3 and returns None at its wildcard. Therefore the keeper cannot produce any selection that contains that valid entry, for current, previous or stable sets. This is deterministic even when C never changes.
guards: VenueExtras remains restricted to authorized setters; PDA canonical bumps, writable flags and denylists remain checked on-chain. A manual caller can provide the valid extras. An active venue requirement for this particular template is not proved; the supported configuration path itself is broken.
impact: The keeper lacks a program-supported extras capability. If an exit needs that configured account, all keeper attempts without it fail until configuration or software changes.
description: The keeper rejects a valid coin creator template, so it cannot send the extras set that the program supports.
fix: Resolve the PumpSwap coin creator from the same cached field that the program reads.
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

LEAD | program: shortwin-keeper | function: decode_zc | bug_class: hardening-version-unchecked | group_key: shortwin-keeper | decode_zc | hardening-version-unchecked
location: services/keeper/src/view.rs:48-55, 274-311, 371-375; programs/shortwin/src/instructions/propose_pool.rs:111-132.
title: The keeper reads account layouts without their version checks
code_smells: The decoder checks owner, discriminator and byte length, but it does not check the layout version.
assumption: Every program-owned account with the existing discriminator uses the compiled layout version.
proof: decode_zc copies the first size_of::<T>() bytes through pod_read_unaligned. load_with_positions uses it for GlobalConfig, Market, MarketVault and VenueExtras without a version test. In contrast, ProposePool explicitly requires CONFIG_VERSION and MARKET_VERSION. A future same-discriminator account of adequate length can be misread by the keeper before the program rejects its action.
guards: Owners and discriminators are checked; today's program controls account creation, and simulation limits execution effects. No current path to create an incompatible version was established.
description: The keeper accepts unknown layout versions, so a future account upgrade can make its state differ from the program's state.
next_check: Add type-specific supported-version checks at decoding or immediately after each typed decode; verify mixed-version upgrade behavior.

LEAD | program: shortwin-keeper | function: next_rank_batch | bug_class: ranked-batch-lost-on-rpc-error | group_key: shortwin-keeper | next_rank_batch | ranked-batch-lost-on-rpc-error
location: services/keeper/src/pool_monitor.rs:690-706; services/keeper/src/keeper.rs:3241-3252 (caller outside the frozen in-scope source bundle).
title: A failed RPC read removes pending rank candidates
code_smells: The helper drains the rank queue before its caller fetches accounts, and the caller returns on an RPC error.
assumption: Every batch removed from rank_queue reaches finish_rank_batch.
proof: For queue [A,B] with a one-pool budget, next_rank_batch drains A. If chain.accounts returns Err, the caller's question mark exits before finish_rank_batch. The next successful tick drains and scores B; finish_rank_batch sees an empty queue and publishes only B. A remains absent until a later discovery. If the failed batch empties the queue, the caller skips the ranking branch on the next tick and retains the previous candidates until rediscovery. The source trace establishes candidate loss, but its deployment impact and outer error recovery were not fully reviewed.
guards: Discovery periodically rebuilds the queue; defaults are 300 seconds normally and 10 seconds during failure/window activity. Simulation still guards every on-chain action. No adversarial way to force the RPC failure was proved.
description: The keeper loses a pending batch after an RPC error; the review does not prove the delay under deployed recovery behavior.
next_check: Review the outer tick error policy, then keep candidates pending until scoring succeeds or restore a failed batch before returning.

LEAD | program: shortwin | function: read_state_boxed_for_exit | bug_class: curve-risk-fee-at-spot | group_key: shortwin | read_state_boxed_for_exit | curve-risk-fee-at-spot
location: programs/shortwin/src/venues/mod.rs:1078-1086; programs/shortwin/src/venues/pump_curve.rs:425-485; programs/shortwin/src/instructions/liquidate.rs:337-354; services/keeper/src/decide.rs:285-303.
title: Curve risk quotes retain the fee resolved at spot
code_smells: Exit reads request the EMA fee only for PumpSwap, while curve pricing resolves its ordinary fee from live reserves.
assumption: A curve's live fee is an adequate basis for hypothetical risk quotes throughout the exit check.
proof: read_state_boxed_for_exit sets ema_fee only when kind==PumpSwap. The curve reader resolves fees::pump_curve(live.vt,live.vq,...) and leaves fee_ppm_at_ema at u32::MAX without ema_fee. liquidate::assess and the keeper carry state.quoter().capped(cap) into the EMA and robust-spot quotes. Thus those risk quotes retain a live-tier input even when their reserve reference is historical. No concrete supported fee-tier movement plus a wrong exit decision was established in this bounded review.
guards: The position's hard fee cap, completed-bucket reference, persistence, warm-up and budget checks still apply. The source explicitly makes the extra EMA calculation PumpSwap-only; extending it to curves needs a policy decision and a concrete economic trace.
description: Curve exits use a live fee for historical risk quotes; the review does not prove that a supported tier change causes a wrong exit.
next_check: Trace a real curve fee schedule across a reachable tier boundary and compare the full liquidation and settlement predicates before proposing any change.
