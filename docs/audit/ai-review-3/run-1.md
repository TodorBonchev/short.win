# Run 1 — rust-auditor

<!--RUN pass=1 of=2 stamp=20261018-110553 sha=b815269 agents=11/12-->

Pass 1 of 2 · 2026-10-18 · `b815269` · 11/12 agents returned.

Coverage loss: specialist 3 (economic-security): tool safety error before final report; no retry in this pass.

## Findings

<!--F key=shortwin|apply-config|unsafe-authority-transfer conf=75 kind=FINDING agents=1-->

[75] **Authority changes accept a successor without its signature**

`shortwin::apply_config` · Confidence: 75

**Description**
An honest governance transfer can install an unusable key, so further configuration changes require a program upgrade.

**Fix**

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

<!--/F-->

<!--F key=shortwin-math|withdraw-in|carry-crosses-empty-pool conf=75 kind=FINDING agents=1-->

[75] **The last withdrawal leaves carry for the next lender**

`shortwin-math::withdraw_in` · Confidence: 75

**Description**
A new lender collects up to 18,446 lamports of earlier carry in the traced initial-deposit cases because the last withdrawal preserves both remainders.

**Fix**

```diff
--- a/crates/shortwin-math/src/pool.rs
+++ b/crates/shortwin-math/src/pool.rs
@@ -1906,6 +1906,15 @@ fn withdraw_in(
     l.shares = sub_u128(l.shares, shares)?;
     p.total_shares = sub_u128(p.total_shares, shares)?;
     if p.total_shares == 0 {
+        l.pending = to_i64(add_i128(i128::from(l.pending), p.carry / WAD_I)?)?;
+        l.real = add_u64(l.real, to_u64(p.carry_r / WAD)?)?;
+        p.dust = add_i128(
+            add_i128(p.dust, p.carry % WAD_I)?,
+            to_i128(p.carry_r % WAD)?,
+        )?;
+        p.carry = 0;
+        p.carry_r = 0;
+
         // AIR-08: the market has no lender again; SOL that reaches the trader from now on is
         // the protocol's (KI-10), so the next deposit waits for the sweeps, as after a wipe.
         p.wipe_swept = 0;
```

<!--/F-->

<!--F key=shortwin|flag-venue-upgraded|unhashed-slot-upgrade-latch conf=75 kind=FINDING agents=1-->

[75] **Hashless venue guards let outsiders block new opens.**

`shortwin::flag_venue_upgraded` · Confidence: 75

**Description**
An outsider can extend a growable venue without a stored ELF hash, then block new opens until Emergency revalidation.

**Fix**

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

<!--/F-->

<!--F key=shortwin-keeper|score|stale-lp-counter conf=75 kind=FINDING agents=2-->

[75] **Correctness: Pool ranks combine LP values from different states**

`shortwin-keeper::score` · Confidence: 75

**Description**
An LP changes liquidity after discovery, so the keeper drops eligible pools or assigns them incorrect ranks.

**Fix (Option A — refresh the counter; partial fix across split RPC snapshots)**

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

**Fix (Option B — read each pool in one RPC snapshot)**

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

<!--/F-->

<!--F key=shortwin-keeper|score|rounded-lock-share-ranking conf=75 kind=FINDING agents=2-->

[75] **Correctness: Pool ranks round positive locked SOL to zero**

`shortwin-keeper::score` · Confidence: 75

**Description**
An LP dilutes the locked share below one basis point, so the keeper excludes a pool whose locked SOL stays unchanged.

**Fix (Option A — divide by a counter bounded to one)**

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

**Fix (Option B — reject zero before raw-ratio division)**

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

<!--/F-->

<!--F key=shortwin-keeper|resolve-entry|unsupported-coin-creator-placeholder conf=75 kind=FINDING agents=1-->

[75] **Correctness: The keeper omits supported coin creator extras**

`shortwin-keeper::resolve_entry` · Confidence: 75

**Description**
The keeper drops every extras set with the supported coin creator placeholder, so it cannot construct those configured exit variants.

**Fix**

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

<!--/F-->

## Leads

<!--F key=shortwin|sync-lender|repeated-accrual-truncation kind=LEAD agents=1-->

- **Repeated sync calls discard fractional lender credits** — `shortwin::sync_lender` — Code smells: per-touch floor; snapshots advance without a lender remainder — An outsider can erase less than one lamport per lender touch; material aggregate harm beyond the disclosed rounding policy remains unverified.

<!--/F-->

<!--F key=shortwin|close-short|hardening-position-status-unchecked kind=LEAD agents=2-->

- **Exit instructions omit the position status check** — `shortwin::close_short` — Code smells: close_short, liquidate, expire, sol_settle omit status guards — A future instruction that persists a non-open status needs matching exit checks; current writers only create open positions.

<!--/F-->

<!--F key=shortwin|inputs|transaction-controlled-settlement-legs kind=LEAD agents=1-->

- **Temporary withdrawals change live settlement inputs** — `shortwin::inputs` — Code smells: live reserves feed S1/S2; only the CR leg uses the EMA floor — An LP changes live settlement inputs with temporary withdrawals; the current arming sequence, borrower loss, and net payoff remain unverified.

<!--/F-->

<!--F key=shortwin|assess|manipulable-liquidity-history kind=LEAD agents=1-->

- **Wrapped observations can preserve a low liquidity estimate** — `shortwin::assess` — Code smells: the liquidation floor trusts a prior EMA; repeated withdrawals can freeze its history — An LP can preserve a low liquidity history; the current observation sequence and forced-liquidation payoff remain unverified.

<!--/F-->

<!--F key=shortwin|deposit|donation-extends-deposit-hold kind=LEAD agents=1-->

- **Small donations can extend a settlement hold** — `shortwin::deposit` — Code smells: the shared vest gates deposits; public sweeps renew the vest — An attacker extends a held vest with small sweeps; the exact settlement setup and a fix that preserves historical rewards remain unverified.

<!--/F-->

<!--F key=shortwin|pricing-inputs|hardening-fixed-supply-fee-tier kind=LEAD agents=1-->

- **The curve adapter uses a fixed supply for fees.** — `shortwin::pricing_inputs` — Code smells: The adapter uses fixed supply for ordinary and EMA fees, live supply for synthetic fees, and a parser that accepts several tiers. — The curve adapter selects the wrong fee after burns if Pump adds tiers; this review does not verify such a current schedule.

<!--/F-->

<!--F key=shortwin-venue-state|read-base|hardening-mint-type-unchecked kind=LEAD agents=1-->

- **The base mint reader omits account type checks.** — `shortwin-venue-state::read_base` — Code smells: read_base checks the owner and minimum length but omits the Token-2022 account type and padding checks. — A future caller that skips mint identity checks can accept token-account bytes as mint supply; current callers use constrained keys.

<!--/F-->

<!--F key=shortwin|parse|hardening-vault-authority-unchecked kind=LEAD agents=2-->

- **The Raydium readers omit vault authority checks.** — `shortwin::parse` — Code smells: Both parse functions check vault keys, token programs, and mints but omit the authorities that their verify functions check. — A future venue authority change can make the readers accept vaults that the swap authority cannot spend; no current transition is proved.

<!--/F-->

<!--F key=shortwin|math-tiers|unbounded-tier-allocation kind=LEAD agents=1-->

- **The adapters copy every fee tier into the heap.** — `shortwin::math_tiers` — Code smells: Both math_tiers functions collect every parsed tier into a Vec, while tiers_at limits the count only by account length. — A fee-config expansion can exhaust the program heap before a reader returns no route; this pass does not prove a failing tier count.

<!--/F-->

<!--F key=shortwin|elf-fingerprint-of|hardening-truncated-elf-digest kind=LEAD agents=2-->

- **The guard keeps twelve bytes of the ELF hash.** — `shortwin::elf_fingerprint_of` — Code smells: elf_fingerprint_of retains 12 SHA-256 bytes, and is_upgrade compares that prefix after ProgramData grows. — A venue authority can seek two ELF files with one fingerprint; this review does not prove a practical collision.

<!--/F-->

<!--F key=shortwin|exit-risk-cap-bps|live-supply-risk-fee kind=LEAD agents=1-->

- **A supply burn can raise the PumpSwap risk fee.** — `shortwin::exit_risk_cap_bps` — Code smells: PumpSwap resolves both live and EMA fee tiers from the current mint supply, so the EMA price does not fix this input. — A holder can burn tokens before liquidation and raise the risk fee; this pass does not prove a profitable liquidation.

<!--/F-->

<!--F key=shortwin|pricing-inputs|live-supply-synthetic-risk-fee kind=LEAD agents=1-->

- **A burn can raise the curve synthetic risk fee.** — `shortwin::pricing_inputs` — Code smells: pricing_inputs resolves synthetic_fees from the current supply, and risk quotes retain it under the position hard cap. — A holder can raise the synthetic risk fee after a burn under multiple tiers; this pass does not prove a liquidation.

<!--/F-->

<!--F key=shortwin|pricing-inputs|synthetic-fee-gate-basis-mismatch kind=LEAD agents=1-->

- **The curve fee gates use a different supply basis.** — `shortwin::pricing_inputs` — Code smells: Ordinary fee gates use the fixed 10^15 supply, while synthetic exits resolve their fee from the live supply. — The curve fee gates can admit a lower fee than a synthetic exit pays; this requires a tier boundary between the supply bases.

<!--/F-->

<!--F key=shortwin|read-state-boxed-for-exit|curve-risk-fee-at-spot kind=LEAD agents=1-->

- **Curve risk quotes retain the fee resolved at spot.** — `shortwin::read_state_boxed_for_exit` — Code smells: read_state_boxed_for_exit enables the EMA fee only for PumpSwap, and ordinary curve risk quotes retain the fee from live reserves. — A keeper can raise the curve risk fee with a sale under multiple tiers; this pass does not prove a liquidation.

<!--/F-->

<!--F key=shortwin|sweep-trader|external-cashback-recovery-dependency kind=LEAD agents=1-->

- **Cashback cleanup depends on external claim instructions.** — `shortwin::sweep_trader` — Code smells: sweep_trader sets the rewards bit only when external cashback balances reach zero, and the keeper relies on venue claim instructions. — A venue upgrade can leave donated cashback unclaimable and keep deposits closed; the current claim interfaces still work.

<!--/F-->

<!--F key=shortwin|deposit|hardening-pool-vault-fields-unchecked kind=LEAD agents=1-->

- **Pool vault loads omit token field constraints.** — `shortwin::deposit` — Code smells: The canonical vault PDA and stored key are checked, but deposit, withdraw, open_short, close_short, liquidate, and expire omit explicit token field constraints. — A future vault importer can admit the wrong asset because the loaders omit checks of the vault mint, authority, and token program.

<!--/F-->

<!--F key=shortwin|init-uva|hardening-existing-uva-validation kind=LEAD agents=1-->

- **Existing Pump UVAs skip full validation at listing.** — `shortwin::init_uva` — Code smells: Both init_uva helpers accept a nonempty account with the correct venue owner before decoding its discriminator or stored user. — A future venue layout change can let a caller list a market with an existing UVA that later fails the stricter trade checks.

<!--/F-->

<!--F key=shortwin-math|pump-curve|hardening-fixed-supply-fee-basis kind=LEAD agents=1-->

- **Normal curve quotes use the original mint supply.** — `shortwin-math::pump_curve` — Code smells: pump_curve resolves fees with PUMP_CURVE_MCAP_SUPPLY, while the synthetic resolver receives the live supply. — A token holder gets inaccurate curve quotes after a burn if Pump adds fee tiers; the source says its current single tier prevents this.

<!--/F-->

<!--F key=shortwin-common|parse-lookup-table|hardening-truncated-metadata-accepted kind=LEAD agents=1-->

- **The ALT parser accepts a truncated metadata header** — `shortwin-common::parse_lookup_table` — Code smells: a missing metadata slice becomes an empty list; the full header length is unchecked — The parser accepts truncated metadata; a future untrusted loader could conceal malformed tables as empty tables.

<!--/F-->

<!--F key=shortwin-keeper|decode-zc|hardening-version-unchecked kind=LEAD agents=2-->

- **The keeper reads layouts without their version checks** — `shortwin-keeper::decode_zc` — Code smells: the owner, discriminator, and byte length are checked; the layout version is unchecked — The keeper accepts account layouts without version checks; a future upgrade may make the decoded fields incompatible.

<!--/F-->

<!--F key=shortwin-keeper|check-foreign|held-liquidity-freeze-residual kind=LEAD agents=1-->

- **Coverage preserves an earlier liquidity low** — `shortwin-keeper::check_foreign` — Code smells: earlier liquidity lows persist after foreign observations; settlement effects remain unverified — An LP can preserve an earlier liquidity low, but this pass does not prove a resulting settlement change or profit.

<!--/F-->

<!--F key=shortwin-keeper|prepare-pamm-trader|stale-external-account-readiness kind=LEAD agents=1-->

- **The keeper retains readiness after external accounts close** — `shortwin-keeper::prepare_pamm_trader` — Code smells: cached readiness skips account reads; recipient ATA closure and repair remain unverified — The keeper trusts cached readiness after accounts close, but the recipient’s ability to close the required ATA remains unverified.

<!--/F-->
