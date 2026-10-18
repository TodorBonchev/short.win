FINDING | program: shortwin | function: apply_config | bug_class: unsafe-authority-transfer | group_key: shortwin | apply_config | unsafe-authority-transfer
title: Authority changes install governance keys without signed acceptance
file: programs/shortwin/src/instructions/apply_config.rs:80
related_locations: programs/shortwin/src/state/config.rs:193; programs/shortwin/src/state/config.rs:769; programs/shortwin/src/instructions/propose_config.rs:19
severity: medium
confidence: high
root_cause: apply_config installs a different governance key from the approved payload without proof that the successor can sign.
guard_gap: ProposeConfig requires the current governance as Signer and compares its key at propose_config.rs:19-25; ApplyConfig has no successor Signer at apply_config.rs:35-55. check_authorities rejects default and duplicate keys, but accepts an unreachable nondefault key.
path: An honest old governance signer proposes Authorities with an unusable successor -> an independent caller waits for not_before -> apply_config commits the successor and clears pending -> no caller can authorize another configuration proposal.
proof: Let C be the existing canonical config PDA, G the live governance signer, P the live pause authority, F a valid System-owned fee recipient, and coverage_authority = Pubkey::default(). Let C, G, P, and F differ. With otherwise valid parameters and timelock_s = 172800, G submits propose_config(Authorities { governance: C, pause_authority: P, coverage_authority: default, fee_recipient: F }) at t0 = 2000000000. ProposeConfig authenticates G; check_authorities checks only nondefault values and pairwise distinctness (config.rs:775-795), so C passes. The proposal stores not_before = 2000172800 (propose_config.rs:34-39). At that second, an unrelated fee payer submits apply_config with C writable, F writable, oracle_cache omitted, and the required event accounts. No governance account or governance signature is required. The time bounds and unchanged valid parameter set pass. GlobalConfig::apply writes self.governance = C (config.rs:193), and apply_config clears pending (apply_config.rs:80-81). F passes the recipient check. C is an off-curve PDA. The source contains no CPI signer seeds for GlobalConfig::SEED and no instruction that restores governance outside initialization or this same proposal flow. Every future propose_config now needs C as a Signer (propose_config.rs:19-25); the old G fails the key check and an unsigned C fails Anchor's signer check. The existing program cannot recover this role. Its upgrade authority can recover only through a program upgrade. The 48-hour delay permits cancellation if humans detect the mistake; it does not prove successor control. Anchor 1.2.0 signer.rs:49-56 explicitly checks info.is_signer. This second pass independently traced the same cause. No build, test, or runtime reproduction was run.
description: An honest admin can install an unusable governance key, so later configuration changes require a program upgrade.
fix: Require the new governance key to sign before apply_config replaces the current governance key.
fix_notes: The optional signer preserves permissionless execution for parameter changes and authority changes that keep governance unchanged. Clients and the IDL must expose the added account. This proposed patch was not compiled.
minimal_fix:
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

FINDING | program: shortwin | function: flag_venue_upgraded | bug_class: unhashed-slot-upgrade-latch | group_key: shortwin | flag_venue_upgraded | unhashed-slot-upgrade-latch
title: Hashless venue guards let outsiders block new opens
file: programs/shortwin/src/instructions/flag_venue_upgraded.rs:126
related_locations: programs/shortwin/src/instructions/initialize_config.rs:159; programs/shortwin/src/instructions/revalidate_venue.rs:74; programs/shortwin/src/state/venue_guard.rs:113; programs/shortwin/src/engine/guard.rs:113
severity: medium
confidence: high
root_cause: A growable venue can have an unflagged guard without an ELF fingerprint, so any changed ProgramData slot can set its upgrade latch.
guard_gap: flag_venue_upgraded checks is_upgrade only when elf_fingerprint returns Some; initialize_config leaves guard_len zero, and hashless revalidate_venue clears it again.
path: Initialize an enabled growable venue without its fingerprint, or clear its fingerprint through an honest revalidation -> an outsider pays to extend ProgramData -> flag_venue_upgraded sets the venue bit -> open_short refuses VenueUpgraded until Emergency revalidation.
proof: Use Raydium CPMM, venue=4, guard_slot[4]=100, upgraded=0, guard_len[2]=0, and an upgradeable ProgramData with at least 10,240 bytes below the 10 MiB size cap. At a later slot 101, the outsider submits the legacy loader ExtendProgram instruction with additional_bytes=10240 and its own payer. Local solana-bpf-loader-program-4.3.0/src/lib.rs:789-790 calls common_extend_program(..., false); lines 923-934 check the upgrade authority only when check_authority is true. Lines 968-971 resize the account, and lines 987-991 write slot=clock_slot. The outsider then passes the real CPMM ProgramData and canonical VenueGuard to flag_venue_upgraded(4). The owner/address/header checks pass; slot 101 differs from guard_slot 100. elf_fingerprint(4) returns None because guard_len[2]=0, so lines 130-135 skip is_upgrade and line 136 sets bit 4. GUARD_MASK_RAYDIUM at constants.rs:146 contains bits 3 and 4, and engine/guard.rs:113-119 rejects either Raydium open while either bit is set. Initialization reaches this state directly; revalidate_venue with expected_slot=current and expected_elf_hash=[0;32] also clears the fingerprint and latch. No venue upgrade signer is needed. The trace assumes the growable ProgramData state identified by this source; no live deployment size was queried. No build, test, or runtime reproduction was run.
guards: The fixed ProgramData address, loader owner, and tag prevent fake accounts. A recorded fingerprint rejects an unchanged ELF extension. Exits remain available.
impact: An outsider blocks new opens and candidate proposals for the extension rent and transaction fees; Emergency revalidation must restore the venue.
description: An outsider can block new opens with a program extension while the venue guard has no ELF fingerprint.
fix: Keep growable venues latched until revalidation supplies an ELF hash, and reject later hashless revalidation.
fix_notes: The patch covers new deployments. Existing deployments also need their growable venues revalidated with hashes before rollout. The proposed patch was not compiled.
minimal_fix:
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

LEAD | program: shortwin | function: close_short | bug_class: hardening-position-status-unchecked | group_key: shortwin | close_short | hardening-position-status-unchecked
title: Exit instructions omit the position status check
file: programs/shortwin/src/instructions/close_short.rs:82
related_locations: programs/shortwin/src/instructions/liquidate.rs:123; programs/shortwin/src/instructions/expire.rs:93; programs/shortwin/src/instructions/sol_settle.rs:122; programs/shortwin/src/instructions/add_margin.rs:63; programs/shortwin/src/state/position.rs:18
code_smells: AddMargin requires position.status == POSITION_OPEN, but CloseShort, Liquidate, Expire, and SolSettle do not.
description: A future instruction that sets POSITION_EXIT_IN_PROGRESS can leave its position eligible for close_short and keeper exits because those paths omit status checks.
limits: This is not an exploit in the reviewed build. POSITION_EXIT_IN_PROGRESS is reserved for v1.1, and open_short currently writes only POSITION_OPEN. The independent gate already exists in add_margin; the other four Accounts structs check the account's borrower, market, and PDA but not its status. A future non-open state must be excluded explicitly before this omission becomes reachable.
fix: Add the POSITION_OPEN constraint to each exit instruction before any instruction writes another live position status.

LEAD | program: shortwin | function: deposit | bug_class: hardening-pool-vault-fields-unchecked | group_key: shortwin | deposit | hardening-pool-vault-fields-unchecked
title: Pool vault loads omit token field constraints
file: programs/shortwin/src/instructions/deposit.rs:94
related_locations: programs/shortwin/src/instructions/create_market.rs:278; programs/shortwin/src/instructions/withdraw.rs:127; programs/shortwin/src/instructions/open_short.rs:166; programs/shortwin/src/engine/trader.rs:479
code_smells: The pool_vault constraints check its PDA, but omit token::mint, token::authority, and token::token_program.
trace: Deposit binds the market to mint, pool_vault, and token_program, then loads pool_vault as InterfaceAccount<TokenAccount>. Its constraints do not compare the token account's embedded mint or authority. create_pool_vault currently creates this exact PDA and initializes it for the named mint and market; outsiders cannot sign for that PDA or change its token authority. Withdraw and the short instructions use the same vault pattern. Token CPIs and trader post-conditions add downstream checks. No current counterfeit-vault path was proved.
description: A future vault migration can admit the wrong token fields because deposit checks the vault address without explicit token field constraints.
fix: Add token mint, authority, and program constraints to each pool_vault load.

LEAD | program: shortwin | function: init_uva | bug_class: hardening-existing-uva-validation | group_key: shortwin | init_uva | hardening-existing-uva-validation
title: Existing UVAs skip full validation during market creation
file: programs/shortwin/src/venues/pump_amm.rs:1161
related_locations: programs/shortwin/src/venues/pump_curve.rs:1047; programs/shortwin/src/venues/pump_amm.rs:469; programs/shortwin/src/venues/pump_curve.rs:766; programs/shortwin/src/instructions/create_market.rs:430
code_smells: Both init_uva helpers accept an existing venue-owned, nonempty UVA without decoding its discriminator or user field.
trace: create_market derives the UVA from the trader and calls init_uva below creator: Signer. Each helper checks the program, UVA key, and event authority. The existing-account branch then checks only the owner and nonempty data. The trading paths decode the UVA and require its user to equal the trader. The current venue PDA rules prevent outsiders from populating a different account type or user there. No current wrong-user creation path was proved.
description: A future venue layout change can make listing accept an invalid UVA that later trades reject.
fix: Decode an existing UVA and require its user to equal the trader before the helper returns.

LEAD | program: shortwin | function: sweep_trader | bug_class: external-cashback-recovery-dependency | group_key: shortwin | sweep_trader | external-cashback-recovery-dependency
title: Cashback cleanup depends on external claim instructions
file: programs/shortwin/src/instructions/sweep_trader.rs:246
related_locations: programs/shortwin/src/engine/lender.rs:197; programs/shortwin/src/venues/pump_amm.rs:791; programs/shortwin/src/instructions/sweep_trader.rs:339
code_smells: sweep_trader sets the rewards bit only after external cashback accounts report no claimable balance.
trace: rewards_required follows the pump UVA or pump venue. SweepTrader reads the keyed pump UVA and, on PumpSwap, its WSOL ATA. A positive claimable amount leaves rewards_clear false, so the VenueRewards sweep does not set WIPE_REWARDS_CLAIMED. The function never invokes the venue claim itself; callers must bundle the external permissionless claim described by its module. The source asserts current venue claims work without the trader signature. This static pass did not inspect deployed venue bytecode or prove an upgrade that disables those claims. The dependency concerns recovery of outstanding cashback after such a change.
description: A future venue change that disables cashback claims can leave deposits waiting for the rewards bit after a pool wipe.
fix: Define and test a recovery path for outstanding cashback when an external claim stops working.

LEAD | program: shortwin | function: elf_fingerprint_of | bug_class: hardening-truncated-elf-digest | group_key: shortwin | elf_fingerprint_of | hardening-truncated-elf-digest
title: The guard stores twelve bytes of each ELF hash
file: programs/shortwin/src/state/venue_guard.rs:47
related_locations: programs/shortwin/src/state/venue_guard.rs:52; programs/shortwin/src/instructions/flag_venue_upgraded.rs:100; programs/shortwin/src/instructions/revalidate_venue.rs:104
code_smells: elf_fingerprint_of retains 12 bytes, and is_upgrade compares only that prefix after ProgramData grows.
trace: GUARD_ELF_LEN=12 yields a 96-bit stored fingerprint. A venue authority that chooses both ELFs can seek a pair with the same prefix, obtain review of one, then extend and replace it with the other. The generic chosen-pair work is about 2^48 hashes; this review supplies no collision, practical search budget, or usable malicious ELF pair. The full 32-byte reviewed hash is present at revalidation but not retained.
description: A venue authority can seek two ELF files with one stored fingerprint; this review does not prove a practical collision.
fix: Store and compare the full SHA-256 digest during venue revalidation and upgrade checks.
