FINDING | program: shortwin | function: apply_config | bug_class: unsafe-authority-transfer | group_key: shortwin | apply_config | unsafe-authority-transfer
location: programs/shortwin/src/instructions/apply_config.rs:80; programs/shortwin/src/state/config.rs:193; programs/shortwin/src/state/config.rs:769
related_locations: programs/shortwin/src/instructions/propose_config.rs:18; programs/shortwin/src/instructions/cancel_config.rs:36
title: Authority changes accept a successor without its signature
boundary: apply_config's stored Authorities change and its successor authority keys.
missing_check: signer; successor acceptance.
assumption: A nondefault, distinct successor key identifies an authority that can use the role.
actual: The handler replaces governance, pause_authority, and coverage_authority without a signature from any successor.
path: Honest governance proposes a mistaken successor G_bad; the timelock elapses; any caller applies the proposal; G_bad becomes governance.
proof: Set G_bad = config.key(), the existing canonical PDA derived from [GlobalConfig::SEED] under shortwin. No production instruction signs governance calls with that PDA. Keep pause_authority, coverage_authority, and fee_recipient distinct and otherwise valid. check_authorities rejects zero and duplicate keys, but it does not authenticate G_bad. propose_config accepts the old governance signature. apply_config checks pending existence, not_before, the 14-day application window, and parameter validity. It calls c.apply and c.pending.clear without any successor account or signature. GlobalConfig::apply assigns self.governance = *governance. The fee_recipient account can remain the valid existing System account, so its additional check passes. The old governance then fails propose_config's equality constraint. The Emergency authority cannot propose a replacement and cannot cancel an Authorities change under cancel_config. No protocol instruction restores the governance key.
guards_and_limits: An outsider cannot create or alter the proposal. The old governance must make an honest address error and fail to cancel it before execution. A program upgrade can repair the state. Public execution of an authenticated proposal is intentional; the defect is the absent successor acceptance.
impact: An honest authority rotation can disable all later governance changes until a program upgrade.
description: Governance can install an unusable successor without its signature, so later governance changes fail until a program upgrade.
fix: Require each changed, nondefault authority to sign before the handler applies an Authorities change.
minimal_diff: The diff is proposed only. It uses remaining accounts for successor signatures and requires matching client metadata.
```diff
--- a/programs/shortwin/src/instructions/apply_config.rs
+++ b/programs/shortwin/src/instructions/apply_config.rs
@@ -77,6 +77,28 @@
         let change = c.pending.change()?;
         c.check_change(&change)?;
         let applied = (c.pending.kind, c.pending.hash());
+        if let ConfigChange::Authorities {
+            governance,
+            pause_authority,
+            coverage_authority,
+            ..
+        } = &change
+        {
+            for (current, next) in [
+                (c.governance, *governance),
+                (c.pause_authority, *pause_authority),
+                (c.coverage_authority, *coverage_authority),
+            ] {
+                if next != Pubkey::default() && next != current {
+                    require!(
+                        ctx.remaining_accounts
+                            .iter()
+                            .any(|account| account.key() == next && account.is_signer),
+                        ShortwinError::Unauthorized
+                    );
+                }
+            }
+        }
         c.apply(&change, clock.unix_timestamp)?;
         c.pending.clear();
         (applied.0, applied.1, change)
```

FINDING | program: shortwin | function: flag_venue_upgraded | bug_class: unhashed-slot-upgrade-latch | group_key: shortwin | flag_venue_upgraded | unhashed-slot-upgrade-latch
location: programs/shortwin/src/instructions/flag_venue_upgraded.rs:128; programs/shortwin/src/instructions/initialize_config.rs:159; programs/shortwin/src/instructions/revalidate_venue.rs:104
related_locations: programs/shortwin/src/state/venue_guard.rs:110; programs/shortwin/src/engine/guard.rs:110; programs/shortwin/src/instructions/open_short.rs:291
title: Hashless venue guards let outsiders block new opens
boundary: flag_venue_upgraded's authentic ProgramData input and its hashless VenueGuard entry.
missing_check: identity of the reviewed executable after an externally authorized ProgramData extension.
assumption: A changed ProgramData slot proves an upgrade when no ELF fingerprint exists.
actual: An outsider can extend an upgradeable program without its authority signature and change the slot without changing the trimmed ELF.
path: Initialize a growable venue with guard_len = 0; extend its authentic ProgramData in a later slot; call flag_venue_upgraded; open_short fails with VenueUpgraded.
proof: Use the CPMM guard index 4 with upgraded = 0, guard_slot[4] = 100, guard_len[2] = 0, and an upgradeable ProgramData shorter than 10 MiB. In slot 101, send loader ExtendProgram with additional_bytes = 10,240, the authentic ProgramData and Program accounts writable, System, and an attacker payer. The pinned solana-bpf-loader-program 4.2.2 dispatches ExtendProgram to common_extend_program(..., false) at lines 793-794. Its authority check only runs when check_authority is true in common_extend_program. It preserves the authority and stores slot = clock_slot at lines 993-996. Then call flag_venue_upgraded(4) with the canonical guard and compile-time CPMM ProgramData key. The address, loader owner, and tag checks pass. slot != guard_slot passes. elf_fingerprint returns None because guard_len[2] is zero, so the handler skips is_upgrade and sets upgraded |= 16. The open guard rejects every venue whose mask includes this bit. initialize_config leaves all guard_len entries zero and upgraded = 0. revalidate_venue accepts an all-zero expected_elf_hash at the current slot and can restore the same vulnerable state.
guards_and_limits: The venue must remain upgradeable and have at least 10,240 bytes of headroom, or the caller must use the permitted smaller final extension. The caller pays any required rent. An immutable or full-sized ProgramData blocks the extension. A nonzero, correct stored fingerprint detects this unchanged ELF. Exits do not consult the latch. This is a static proof against the repository's pinned loader source; no live feature state or deployment was tested.
impact: An outsider blocks new opens and affected pool selection until Emergency revalidation records a fingerprint.
description: An outsider extends a venue with no stored ELF fingerprint, changes its deploy slot, and blocks new opens through the upgrade latch.
fix: Keep growable venues blocked until revalidation records a verified ELF fingerprint, and refuse later hashless revalidation for those venues.
minimal_diff: This proposed startup fix changes no account layout. Existing deployments with empty fingerprints need an explicit bootstrap migration before reopening.
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

LEAD | program: shortwin | function: init_uva | bug_class: hardening-existing-uva-validation | group_key: shortwin | init_uva | hardening-existing-uva-validation
location: programs/shortwin/src/venues/pump_amm.rs:1161; programs/shortwin/src/venues/pump_curve.rs:1047
related_locations: programs/shortwin/src/instructions/create_market.rs:430; programs/shortwin/src/venues/pump_amm.rs:469
title: Existing Pump UVAs skip full validation at listing
boundary: The existing UVA branch of each create_market CPI helper.
code_smells: Both helpers accept the expected UVA key when its owner matches the venue and its data is nonempty.
proof: Neither existing-account branch checks the UVA discriminator, minimum layout, or stored user. Both return Ok immediately. The PumpSwap trade check later calls uva_ok, which parses the complete UVA and compares u.user with market.trader.
guards_and_limits: create_market derives the UVA from the canonical trader PDA. Both helpers check the exact fixed executable venue program and event authority. The creator signs as payer. An outsider cannot write arbitrary data at this venue PDA. No current account substitution or funds loss was proved. A changed venue layout could make listing succeed while later trading fails.
description: Listing accepts an existing UVA after only its key, owner, and nonempty-data checks; a future venue layout can pass listing but fail trading.
fix: Validate the existing UVA discriminator, layout, and user before the helper returns success.

LEAD | program: shortwin | function: sweep_trader | bug_class: external-cashback-recovery-dependency | group_key: shortwin | sweep_trader | external-cashback-recovery-dependency
location: programs/shortwin/src/instructions/sweep_trader.rs:248; programs/shortwin/src/instructions/sweep_trader.rs:334; programs/shortwin/src/engine/lender.rs:202
related_locations: programs/shortwin/src/instructions/deposit.rs:152; crates/shortwin-math/src/pool.rs:2027
title: Cashback cleanup depends on external claim instructions
boundary: The trader's authenticated Pump UVA and PumpSwap cashback ATA in sweep_trader's remaining accounts.
code_smells: The sweep requires empty external cashback accounts before it sets WIPE_REWARDS_CLAIMED, but it cannot claim the cashback itself.
proof: With total_shares = 0, wipe_swept = 0, and one claimable cashback unit, rewards_clear is false. sweep_trader sets the trader-swept bit but skips SweepSource::VenueRewards. rewards_required remains true on pump venues and on any market that retains pump_uva. deposit passes that requirement into the math pool, which rejects a missing reward bit with WipeSweepPending. Only an external claim that removes the cashback can enable the next sweep to set the bit.
guards_and_limits: Current documented Pump and PumpSwap claim instructions accept an unsigned trader. The source describes a working external claim plus sweep sequence. This review did not prove a current pause, upgrade, or account state that permanently prevents those claims. This lead concerns the dependency when that external interface stops working. Sweeps constrain every cashback key to the market cache; the caller cannot substitute an empty account.
description: Deposits still need external cashback claims after a wipe; this review did not prove a current state that permanently blocks those claims.
fix: Define a bounded recovery path for external rewards that remain unclaimable after a venue interface change.

LEAD | program: shortwin | function: close_short | bug_class: hardening-position-status-unchecked | group_key: shortwin | close_short | hardening-position-status-unchecked
location: programs/shortwin/src/instructions/close_short.rs:91; programs/shortwin/src/instructions/liquidate.rs:129; programs/shortwin/src/instructions/expire.rs:90; programs/shortwin/src/instructions/sol_settle.rs:128
related_locations: programs/shortwin/src/instructions/add_margin.rs:65; programs/shortwin/src/state/position.rs:20
title: Exit instructions omit the position status check
boundary: ShortPosition accounts in close_short and its sibling exit instructions.
code_smells: Exits validate owner, discriminator, market, borrower, and PDA, but they omit the POSITION_OPEN condition that add_margin enforces.
proof: ShortPosition defines status 0 as open and status 1 as exit in progress. close_short loads the position and trades without checking status. The sibling exit account constraints also omit the condition. add_margin explicitly requires position.status == POSITION_OPEN.
guards_and_limits: The source reserves status 1 for v1.1. The only production status assignment found writes POSITION_OPEN. Anchor 1.2.0 closes accounts by clearing their lamports, assigning System ownership, and resizing data to zero. No present double exit or closed-account revival was proved.
description: Exit instructions omit the position status check; a future instruction that writes another status can make this omission unsafe.
fix: Require POSITION_OPEN at every exit boundary before the handler uses a position.

LEAD | program: shortwin | function: deposit | bug_class: hardening-pool-vault-fields-unchecked | group_key: shortwin | deposit | hardening-pool-vault-fields-unchecked
location: programs/shortwin/src/instructions/deposit.rs:94
related_locations: programs/shortwin/src/instructions/open_short.rs:171; programs/shortwin/src/instructions/close_short.rs:107; programs/shortwin/src/instructions/create_market.rs:281
title: Pool vault loads omit token field constraints
boundary: The pool_vault InterfaceAccount in deposit and the reviewed trading instruction account structs.
code_smells: The vault has PDA and cached-key checks, but its constraints omit token::mint, token::authority, and token::token_program.
proof: Anchor's InterfaceAccount accepts token accounts owned by either supported token program. The pool_vault field then constrains only its address and stored bump. deposit sends tokens into it and checks the resulting amount; it does not restate its token authority. The open_short and close_short account structs use the same form.
guards_and_limits: create_market creates the vault at the derived PDA, assigns the selected token program, and initializes the base mint with market authority. No caller can replace that PDA today. transfer_checked checks mint compatibility on the actual transfer. No current wrong-vault transfer was proved. A future vault migration or authority mutation would remove the initialization-only guarantee.
description: Pool vault loads rely on their creation checks for token fields; a future vault migration can make this missing validation unsafe.
fix: Constrain every pool vault to the market mint, market authority, and selected token program.

LEAD | program: shortwin | function: parse | bug_class: hardening-vault-authority-unchecked | group_key: shortwin | parse | hardening-vault-authority-unchecked
location: programs/shortwin/src/venues/raydium_cpmm.rs:169; programs/shortwin/src/venues/raydium_v4.rs:146
related_locations: programs/shortwin/src/venues/raydium_cpmm.rs:346; programs/shortwin/src/venues/raydium_v4.rs:248
title: The Raydium readers omit vault authority checks
boundary: Authenticated Raydium ReadSet vaults in the normal live-state parsers.
code_smells: Both parsers check account programs, pool-recorded keys, and token mints, but they omit each token account's authority.
proof: raydium_cpmm::parse compares v0.mint and v1.mint with the pool fields, then constructs Parsed without comparing their owner fields. raydium_v4::parse does the same for coin and pc. Their verify functions separately require t.owner == the compiled Raydium authority at listing and candidate adoption.
guards_and_limits: The live ReadSet keys come from the authenticated market cache, and initial adoption checks authorities. A caller cannot alter another program's token authority or substitute another key. No current external instruction that changes a listed vault's authority was established. A future venue change could make the normal readers accept balances the venue cannot spend.
description: The live Raydium readers omit vault authority checks; a future authority change can make them accept reserves that the venue cannot spend.
fix: Require the compiled venue authority in both live vault parsers.

LEAD | program: shortwin | function: elf_fingerprint_of | bug_class: hardening-truncated-elf-digest | group_key: shortwin | elf_fingerprint_of | hardening-truncated-elf-digest
location: programs/shortwin/src/state/venue_guard.rs:48; programs/shortwin/src/state/venue_guard.rs:52
related_locations: programs/shortwin/src/instructions/flag_venue_upgraded.rs:107
title: The guard keeps twelve bytes of the ELF hash
boundary: The fingerprint used to distinguish a ProgramData extension from an upgrade after the account grows.
code_smells: elf_fingerprint_of copies only the first 12 bytes of the SHA-256 digest into persisted state.
proof: GUARD_ELF_LEN is 12. elf_fingerprint_of iterates over that 12-byte destination, so bytes 12 through 31 never participate in the later comparison.
guards_and_limits: A fixed reviewed fingerprint requires a generic 96-bit second-preimage search. An actor who chooses both executable versions has a generic 48-bit collision bound. The attacker also needs a valid deployable executable, authority to upgrade the venue, and an account-length change. No colliding pair or present exploit was constructed. This is separate from the proven empty-fingerprint path.
description: The guard stores only twelve hash bytes; this review did not produce two valid executables that share those bytes.
fix: Store and compare the full SHA-256 digest through an explicit account-layout migration.
