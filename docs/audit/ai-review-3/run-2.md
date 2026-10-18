# Run 2 — rust-auditor

<!--RUN pass=2 of=2 stamp=20261018-110553 sha=b815269 agents=12/12-->

Pass 2 of 2 · 2026-10-18 · `b815269` · 12/12 agents returned.

## Findings

<!--F key=shortwin-math|withdraw-in|carry-crosses-empty-pool conf=75 kind=FINDING agents=3-->

[75] **The last withdrawal leaves carry for the next lender**

`shortwin-math::withdraw_in` · Confidence: 75

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

<!--/F-->

<!--F key=shortwin|apply-config|unsafe-authority-transfer conf=75 kind=FINDING agents=4-->

[75] **Authority changes accept a successor without its signature**

`shortwin::apply_config` · Confidence: 75

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

<!--/F-->

<!--F key=shortwin|flag-venue-upgraded|unhashed-slot-upgrade-latch conf=75 kind=FINDING agents=2-->

[75] **Hashless venue guards let outsiders block new opens**

`shortwin::flag_venue_upgraded` · Confidence: 75

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

<!--/F-->

<!--F key=shortwin-keeper|score|stale-lp-counter conf=75 kind=FINDING agents=1-->

[75] **Correctness: Pool ranks combine LP values from different states**

`shortwin-keeper::score` · Confidence: 75

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

<!--/F-->

<!--F key=shortwin-keeper|score|rounded-lock-share-ranking conf=75 kind=FINDING agents=1-->

[75] **Correctness: Pool ranks round positive locked SOL to zero**

`shortwin-keeper::score` · Confidence: 75

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

<!--/F-->

<!--F key=shortwin-keeper|evaluate|wrong-window-floor-reference conf=75 kind=FINDING agents=1-->

[75] **Correctness: The keeper checks the proposal floor at the wrong price**

`shortwin-keeper::evaluate` · Confidence: 75

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

<!--/F-->

<!--F key=shortwin-keeper|plan|false-window-floor-monotonicity conf=75 kind=FINDING agents=1-->

[75] **Correctness: The keeper skips pools after a floor failure**

`shortwin-keeper::plan` · Confidence: 75

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

<!--/F-->

<!--F key=shortwin-keeper|resolve-entry|unsupported-coin-creator-placeholder conf=75 kind=FINDING agents=2-->

[75] **Correctness: The keeper omits supported coin creator extras**

`shortwin-keeper::resolve_entry` · Confidence: 75

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

<!--/F-->

## Leads

<!--F key=shortwin|sync-lender|repeated-accrual-truncation kind=LEAD agents=3-->

- **Repeated sync calls discard fractional lender credits** — `shortwin::sync_lender` — Code smells: per-touch floor; snapshots advance without a lender remainder — Unsigned syncs discard fractional lender credits; a complete two-release trace loses one lamport, while material aggregate harm remains unproved.

<!--/F-->

<!--F key=shortwin|close-short|hardening-position-status-unchecked kind=LEAD agents=4-->

- **Exit instructions omit the position status check** — `shortwin::close_short` — Code smells: close_short, liquidate, expire, sol_settle omit status guards — A future instruction that persists a non-open status needs matching exit checks; current writers only create open positions.

<!--/F-->

<!--F key=shortwin|deposit|hardening-pool-vault-fields-unchecked kind=LEAD agents=4-->

- **Pool vault loads omit token field constraints.** — `shortwin::deposit` — Code smells: The canonical vault PDA and stored key are checked, but deposit, withdraw, open_short, close_short, liquidate, and expire omit explicit token field constraints. — A future vault importer can admit the wrong asset because the loaders omit checks of the vault mint, authority, and token program.

<!--/F-->

<!--F key=shortwin|inputs|transaction-controlled-settlement-legs kind=LEAD agents=2-->

- **Temporary withdrawals change live settlement inputs** — `shortwin::inputs` — Code smells: live reserves feed S1/S2; only the CR leg uses the EMA floor — Temporary LP withdrawals alter live settlement legs, but a complete production sequence and loss beyond the documented AIR-20 residual remain unproved.

<!--/F-->

<!--F key=shortwin|deposit|donation-extends-deposit-hold kind=LEAD agents=3-->

- **Small donations can extend a settlement hold** — `shortwin::deposit` — Code smells: the shared vest gates deposits; public sweeps renew the vest — Small donations can renew an active settlement hold; sustained blockage and a fix that preserves historical rewards remain unverified.

<!--/F-->

<!--F key=shortwin|init-uva|hardening-existing-uva-validation kind=LEAD agents=2-->

- **Existing Pump UVAs skip full validation at listing** — `shortwin::init_uva` — Code smells: Both init_uva helpers return for a nonempty venue-owned UVA without decoding its discriminator, layout or user. — Listing accepts existing UVAs without decoding their user. A future venue layout can pass listing but fail trading; no current wrong-user path is proved.

<!--/F-->

<!--F key=shortwin|sweep-trader|external-cashback-recovery-dependency kind=LEAD agents=3-->

- **Cashback cleanup depends on external claim instructions** — `shortwin::sweep_trader` — Code smells: sweep_trader leaves the reward bit clear while external cashback remains; it reads the cashback accounts but cannot empty them. — Unclaimable external cashback can keep deposits blocked after a wipe or the last withdrawal. No current failure of the external claim is proved.

<!--/F-->

<!--F key=shortwin|elf-fingerprint-of|hardening-truncated-elf-digest kind=LEAD agents=2-->

- **The guard keeps twelve bytes of the ELF hash** — `shortwin::elf_fingerprint_of` — Code smells: elf_fingerprint_of stores only the first twelve SHA-256 bytes; is_upgrade compares that prefix after the account length changes. — The guard compares twelve hash bytes after ProgramData grows. No practical pair of valid executable files with the same prefix is proved.

<!--/F-->

<!--F key=shortwin-venue-state|read-base|hardening-mint-type-unchecked kind=LEAD agents=1-->

- **The base mint reader omits account type checks** — `shortwin-venue-state::read_base` — Code smells: read_base accepts any length of at least 82 bytes under a token owner and omits mint padding and account type checks. — A future caller without a trusted mint key can read token-account bytes as mint supply. Current caller keys block that substitution.

<!--/F-->

<!--F key=shortwin|parse|hardening-vault-authority-unchecked kind=LEAD agents=2-->

- **The Raydium readers omit vault authority checks** — `shortwin::parse` — Code smells: Both live parsers check vault keys, token programs and mints but omit the authority comparison that their adoption verifiers require. — The live readers omit vault authority checks. A future venue-authorized change can leave accepted reserves unusable; no current public authority-change path is proved.

<!--/F-->

<!--F key=shortwin|read-state-boxed-for-exit|curve-risk-fee-at-spot kind=LEAD agents=1-->

- **Curve risk quotes retain the fee resolved at spot** — `shortwin::read_state_boxed_for_exit` — Code smells: Exit reads request an EMA fee only for PumpSwap, while ordinary curve fees come from live reserves. — Curve exits retain the live fee in historical risk quotes. No supported tier change and resulting wrong exit decision is proved.

<!--/F-->

<!--F key=shortwin-math|pump-curve|hardening-fixed-supply-fee-basis kind=LEAD agents=1-->

- **Normal curve quotes use the original mint supply** — `shortwin-math::pump_curve` — Code smells: pump_curve uses PUMP_CURVE_MCAP_SUPPLY for ordinary fees; only the separate synthetic resolver receives the live supply. — Normal curve quotes can choose a different fee after a burn under multiple tiers. The documented single-tier schedule prevents this today.

<!--/F-->

<!--F key=shortwin-keeper|decode-zc|hardening-version-unchecked kind=LEAD agents=3-->

- **The keeper reads layouts without their version checks** — `shortwin-keeper::decode_zc` — Code smells: The decoder checks the owner, discriminator and byte length, but it omits supported-version checks. — The keeper accepts layouts without version checks; a future upgrade can make its decoded fields incompatible.

<!--/F-->

<!--F key=shortwin-keeper|next-rank-batch|ranked-batch-lost-on-rpc-error kind=LEAD agents=1-->

- **A failed RPC read removes pending rank candidates** — `shortwin-keeper::next_rank_batch` — Code smells: The helper drains the rank queue before the RPC read; the caller returns on error without restoring the removed candidates. — The keeper loses a pending batch after an RPC error until rediscovery. No outsider trigger or stranded funds are proved.

<!--/F-->
