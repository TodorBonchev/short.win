FINDING | program: keeper | function: score | bug_class: stale-lp-counter | group_key: keeper | score | stale-lp-counter
file: services/keeper/src/pool_monitor.rs:252
location: keeper::score
title: Pool ranking combines LP values from different states.
root_cause: CandidatePool stores the pool's internal LP counter at discovery, but score compares it with a later LP mint supply.
assumption: The pool's internal LP counter stays constant between discovery and ranking, even though deposits and withdrawals change it.
violation: A depositor changes both the internal counter and circulating supply after discovery; the keeper refreshes only the supply.
path: Discover an eligible Raydium pool -> deposit liquidity before its rank read -> score compares the old counter with the new mint supply -> rank discards the pool -> keeper never evaluates or proposes that candidate until a later complete discovery and ranking.
proof: services/keeper/src/pool_monitor.rs:166-224 copies lp_amount or lp_supply into CandidatePool.lp_counter. Its rank_keys at lines 228-230 returns only the WSOL vault and LP mint. keeper.rs:3222-3252 calls discover first, then reads those two keys separately and calls score; large discovered sets are ranked over multiple ticks. Let the discovered pool have counter C0=1,000,000,000,000, circulating supply S0=500,000,000,000, and reserves X0=Y0=1,000,000,000,000. The locked LP amount is 500,000,000,000. A proportional deposit of 1,000,000,000,000 LP changes the state to C1=2,000,000,000,000, S1=1,500,000,000,000, X1=Y1=2,000,000,000,000. The actual locked depth stays 500,000,000,000, and the locked SOL stays 500 SOL. However, lambda_bps(C0,S1)=0 because checked_sub fails. score returns 0, and rank at lines 257-260 removes the pool before the verifier sees it. The inverse withdrawal also inflates a score: a discovered counter of 2e12 and current supply 999,900,000,000 gives 5,000 bps, although the current counter 1e12 gives only 1 bp. The research note docs/research/raydium-amm.md:667 confirms that internal counters grow on mint and shrink on withdrawal. The on-chain adapter computes locked_depth from a fresh pool counter and mint supply (programs/shortwin/src/venues/raydium_cpmm.rs:240-243); its validation cannot repair an off-chain candidate already discarded.
impact: This proves false candidate removal and false top-eight selection. Repeated liquidity changes can keep an eligible fallback out of the keeper's candidate set. Other callers can still propose it, and no downstream financial loss is proved here. Default rediscovery is 300 seconds, or 10 seconds on a failing incumbent, and an unfinished ranking delays rediscovery.
description: A depositor changes the LP supply after discovery, so the keeper drops a valid pool or assigns it a false rank.
fix: Read the pool counter with the LP mint and vault before each score.
minimal_unified_diff:
```diff
--- a/services/keeper/src/pool_monitor.rs
+++ b/services/keeper/src/pool_monitor.rs
@@ -225,9 +225,9 @@
         })
     }
 
-    /// The accounts [`rank`] reads for it: its WSOL vault and its LP mint.
-    pub fn rank_keys(&self) -> [Pubkey; 2] {
-        [self.sol_vault, self.lp_mint]
+    /// The accounts [`rank`] reads: its WSOL vault, LP mint and current pool state.
+    pub fn rank_keys(&self) -> [Pubkey; 3] {
+        [self.sol_vault, self.lp_mint, self.pool]
     }
 }
 
@@ -241,6 +241,16 @@
     let (Some(Some(vault)), Some(Some(lp))) = (accounts.first(), accounts.get(1)) else {
         return 0;
     };
+    let Some(fresh) = accounts
+        .get(2)
+        .and_then(Option::as_ref)
+        .and_then(|a| CandidatePool::from_account(c.kind, c.pool, a))
+    else {
+        return 0;
+    };
+    if !fresh.plausible || fresh.read_keys != c.read_keys {
+        return 0;
+    }
     let r = vault
         .data
         .get(64..72)
@@ -249,7 +259,7 @@
     let Ok(m) = mint::read_base(&lp.owner.to_bytes(), &lp.data) else {
         return 0;
     };
-    u128::from(lambda_bps(c.lp_counter, m.supply)) * u128::from(r) / 10_000
+    u128::from(lambda_bps(fresh.lp_counter, m.supply)) * u128::from(r) / 10_000
 }
 
 /// The best `k` candidates by score (highest first; among equals the lowest key), each with a
@@ -688,7 +698,7 @@
 
     /// The next pools to rank within `budget_keys` keys (at least one pool).
     pub fn next_rank_batch(&mut self, budget_keys: usize) -> Vec<CandidatePool> {
-        let n = (budget_keys / 2).max(1).min(self.rank_queue.len());
+        let n = (budget_keys / 3).max(1).min(self.rank_queue.len());
         self.rank_queue.drain(..n).collect()
     }
 
--- a/services/keeper/src/keeper.rs
+++ b/services/keeper/src/keeper.rs
@@ -3243,7 +3243,7 @@
                 let accs = self.chain.accounts(&keys).await?;
                 let scored: Vec<(u128, pool_monitor::CandidatePool)> = batch
                     .into_iter()
-                    .zip(accs.chunks(2))
+                    .zip(accs.chunks(3))
                     .map(|(c, a)| (pool_monitor::score(&c, a), c))
                     .collect();
                 let t = self.pool_tracks.entry(key).or_default();
--- a/services/keeper/src/config.rs
+++ b/services/keeper/src/config.rs
@@ -362,7 +362,7 @@
     /// M4-SEC-7: the best candidates (by locked SOL) evaluated each tick; the rest of a mint's
     /// pools — anyone can create them — are never fetched per tick: 8.
     pub max_candidates: usize,
-    /// M4-SEC-7: the accounts (two per pool: its WSOL vault and LP mint) a discovery's ranking
+    /// M4-SEC-7: the accounts (three per pool: WSOL vault, LP mint, pool) a discovery's ranking
     /// reads per tick and market; a larger discovery is ranked over several ticks: 200.
     pub rank_keys_per_tick: usize,
     /// M7-SEC-8: the most markets whose pool discovery (four `getProgramAccounts` calls each) runs
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
validation: Static source and arithmetic trace only; no builds, tests, chain calls, or repository writes. The production diff changes ranking reads to three accounts; existing ranking fixtures must supply the third account.

FINDING | program: keeper | function: score | bug_class: premature-lock-share-rounding | group_key: keeper | score | premature-lock-share-rounding
file: services/keeper/src/pool_monitor.rs:252
location: keeper::score
title: Pool ranking rounds positive locked SOL to zero.
root_cause: score floors the locked LP fraction to whole basis points before it multiplies by the SOL reserve.
assumption: A whole-basis-point approximation preserves the locked SOL that determines candidate ranking.
violation: A proportional deposit lowers a positive locked fraction just below one basis point while its locked SOL stays unchanged.
path: Eligible Raydium pool with a one-basis-point locked fraction -> tiny proportional deposit -> fresh discovery and ranking -> score becomes zero -> rank removes the candidate despite unchanged locked SOL and locked depth.
proof: This defect needs no stale snapshot. Let all current values be counter C=1,000,000,000,000,000, supply S=999,900,000,000,000, X=Y=1,000,000,000,000,000, and P_ref=10^18. The pool has 100,000,000,000 permanently locked LP and 100 SOL of locked depth/SOL. lambda_bps(C,S)=1, so score=100,000,000,000 lamports. A proportional deposit of 1,000 LP and 1,000 units of each reserve gives C'=1,000,000,000,001,000, S'=999,900,000,001,000, X'=Y'=1,000,000,000,001,000. The exact locked depth remains 100,000,000,000, but floor((C'-S')*10,000/C')=0. score=0 and rank removes it. With ordinary executable status and fees, the exact locked depth exceeds the 25-SOL window floor and the default listing floor. crates/shortwin-math/src/pool_select.rs:29-53 explicitly uses the raw ratio to prevent this same unlocked-deposit failure on chain; the keeper still uses lambda_bps. This concrete zero case requires a pool with substantial existing unlocked liquidity. Smaller boundary crossings also perturb rankings without reducing them to zero.
impact: The candidate remains absent even after rediscovery while the diluted fraction stays below one basis point. The program still accepts a direct valid proposal. The proven impact is keeper selection failure, not an on-chain eligibility bypass or proven asset loss.
description: A liquidity provider adds redeemable liquidity, so the keeper removes an eligible pool even though its locked SOL stays unchanged.
fix: Multiply the raw locked LP amount by the SOL reserve before division by the counter.
minimal_unified_diff:
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
 
@@ -249,7 +249,8 @@
     let Ok(m) = mint::read_base(&lp.owner.to_bytes(), &lp.data) else {
         return 0;
     };
-    u128::from(lambda_bps(c.lp_counter, m.supply)) * u128::from(r) / 10_000
+    u128::from(c.lp_counter.saturating_sub(m.supply)) * u128::from(r)
+        / u128::from(c.lp_counter.max(1))
 }
 
 /// The best `k` candidates by score (highest first; among equals the lowest key), each with a
```
validation: Static arithmetic proof only. This patch is independent against the pinned source; combine it with fresh.lp_counter when applying the stale-lp-counter patch. No tests or builds ran.

FINDING | program: keeper | function: resolve_entry | bug_class: unsupported-coin-creator-placeholder | group_key: keeper | resolve_entry | unsupported-coin-creator-placeholder
file: services/keeper/src/venue.rs:675
location: keeper::resolve_entry
title: The keeper drops valid coin creator extras.
root_cause: resolve_entry omits PLACEHOLDER_COIN_CREATOR even though the on-chain validator and parser support it from the PumpSwap cache.
assumption: The coin creator cannot be resolved from the market cache.
violation: A valid PumpSwap market already contains CACHED_COIN_CREATOR, and the on-chain parser uses that exact cached value.
path: Emergency or Governance publishes a valid template with coin_creator -> keeper decodes VenueExtras -> resolve_entry returns None -> choice_of returns None -> extras_candidates omits the set -> keeper cannot build the supported configured exit variant.
proof: Construct one ExtraEntry with kind=ENTRY_TEMPLATE, n_seeds=1, seeds[0].kind=SEED_PLACEHOLDER, seeds[0].len=PLACEHOLDER_COIN_CREATOR (4), zero seed bytes, a nonzero external program key, and a derived account outside the denylists. programs/shortwin/src/venues/mod.rs::check_entry accepts placeholder IDs through PLACEHOLDER_COIN_CREATOR. set_venue_extras validates and writes that entry. The program's Placeholders::key resolves ID 4, and venues/mod.rs:1264-1268 supplies pump_amm::cached(market, CACHED_COIN_CREATOR) on PumpSwap. The keeper's venue.rs:670-677 handles IDs for mint, pool, trader and bonding_curve only; ID 4 reaches `_ => None`, despite the next comment claiming that the cache cannot supply it. At lines 703-704 choice_of propagates None; extras_candidates lines 738-746 drops current, previous and stable independently if they contain this template. With otherwise empty sets, its result is only ExtrasChoice::none(), even though the configured set is valid on chain.
impact: This proves unsupported configured functionality for every coin_creator template. A venue interface that requires that account defeats the intended extras-only recovery of keeper exits. The current deployed venue was not proved to require such an extra; no present liquidation loss is claimed.
description: The keeper omits every extras set with the supported coin creator placeholder, so it cannot build those configured exits.
fix: Resolve PLACEHOLDER_COIN_CREATOR from the same PumpSwap cache as the program.
minimal_unified_diff:
```diff
--- a/services/keeper/src/venue.rs
+++ b/services/keeper/src/venue.rs
@@ -58,8 +58,8 @@
 use shortwin::state::pump_swap_cache;
 use shortwin::state::{
     ENTRY_FIXED, ENTRY_TEMPLATE, EXTRAS_MAX_SEEDS, ExtraEntry, ExtrasSet, Market,
-    PLACEHOLDER_BONDING_CURVE, PLACEHOLDER_MINT, PLACEHOLDER_POOL, PLACEHOLDER_TRADER, SEED_CONST,
-    SEED_PLACEHOLDER, VenueExtras, VenueKind,
+    PLACEHOLDER_BONDING_CURVE, PLACEHOLDER_COIN_CREATOR, PLACEHOLDER_MINT, PLACEHOLDER_POOL,
+    PLACEHOLDER_TRADER, SEED_CONST, SEED_PLACEHOLDER, VenueExtras, VenueKind,
 };
 use shortwin::venues::{
     EXTRAS_SET_CURRENT, EXTRAS_SET_PREVIOUS, EXTRAS_SET_STABLE, ExtrasSel, pump_amm, pump_curve,
@@ -672,7 +672,9 @@
                             PLACEHOLDER_POOL => pool_placeholder(kind, market),
                             PLACEHOLDER_TRADER => Some(market.trader),
                             PLACEHOLDER_BONDING_CURVE => Some(market.bonding_curve),
-                            // The coin creator is not resolvable from the market cache.
+                            PLACEHOLDER_COIN_CREATOR if kind == VenueKind::PumpSwap => {
+                                Some(pump_amm::cached(market, pump_swap_cache::CACHED_COIN_CREATOR))
+                            }
                             _ => None,
                         }?;
                         owned.push(key.to_bytes().to_vec());
```
validation: Static cross-file trace only. The patch mirrors the program's cached value, including its venue guard; it does not substitute the live creator. No tests or builds ran.

LEAD | program: shortwin | function: sweep_trader | bug_class: external-cashback-recovery-dependency | group_key: shortwin | sweep_trader | external-cashback-recovery-dependency
file: programs/shortwin/src/instructions/sweep_trader.rs:248
location: shortwin::sweep_trader
title: Cashback cleanup still depends on venue claims.
code_smells: The sweep clears the rewards bit only after external cashback balances reach zero; the keeper relies on venue claim instructions to empty them.
assumption: The pump and PumpSwap cashback claim interfaces remain available after a market wipe or last-lender exit.
violation: A venue changes or removes its claim interface while cashback exists, or a stranger donates cashback after that change.
path: Wipe or last-lender exit clears source bits -> someone funds the pump UVA or PumpSwap cashback ATA -> claim fails under the changed venue -> sweep never marks rewards clear -> deposit rejects WipeSweepPending.
proof: sweep_trader.rs:248-261 computes rewards_clear from the pump UVA lamports above rent and PumpSwap cashback ATA balance. The handler only sets SweepSource::VenueRewards when that predicate holds. engine/lender.rs::rewards_required requires this bit for pump venues and any market with a pump UVA; math pool.rs::deposit_in rejects p.wipe_swept & need != need. keeper sweep.rs:35-43 explicitly records that a WSOL donation turns removal of pAMM claim_cashback into a lasting deposit lock. This source disclosure does not supply a recovery path. The pump comments that no CPI means no interface deadlock overstate what off-chain bundling guarantees.
description: A venue upgrade can leave donated cashback unclaimable and keep deposits closed; the current claim interfaces still work.
unverified: No currently deployed failing claim interface is proved. This is a retained dependency lead, not a present exploit finding; a safe fallback needs a protocol decision about old lenders' cashback.

LEAD | program: keeper | function: check_foreign | bug_class: held-liquidity-freeze-residual | group_key: keeper | check_foreign | held-liquidity-freeze-residual
file: services/keeper/src/coverage.rs:47
location: keeper::check_foreign
title: Coverage preserves a previously observed liquidity low.
code_smells: The keeper detects foreign bucket extremes but cannot restore a liquidity low that earlier genuine observations established.
assumption: Honest per-second observations are enough to remove the effects of an LP's temporary withdrawal.
violation: An LP first holds a withdrawal through genuine observations, then wraps later cranks with withdrawals and redeposits.
proof: coverage.rs:36-50 documents that the keeper's liquidity window holds the EMA but cannot lift a low reached during a genuine withdrawal; the low stays frozen at one attacker transaction per keeper observation (FITX-1, O18). check_foreign compares the bucket against that second's depth view and starts per-second coverage, without a separate recovery rule for a genuinely established low. The source calls this an accepted residual; acceptance is not a code guard.
description: An LP can preserve a previously observed liquidity low; this pass did not independently prove the resulting settlement or profit.
unverified: The full observation, liquidation and settlement sequence and its current bounds were not independently reproduced in this pass. Retained because the assigned source expressly reaches the known residual.
