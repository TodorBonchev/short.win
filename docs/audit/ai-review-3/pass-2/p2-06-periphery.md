FINDING | program: shortwin | function: apply_config | bug_class: unsafe-authority-transfer | group_key: shortwin | apply_config | unsafe-authority-transfer
title: Authority changes accept a successor without its signature
location: programs/shortwin/src/instructions/apply_config.rs:77; programs/shortwin/src/state/config.rs:187
root_cause: The handler applies new authority keys without proof that their owners accept the roles.
description: Governance can install an unusable successor key, so later configuration changes fail until a program upgrade restores control.
path: Honest governance proposes an Authorities change with an incorrect governance PDA; the timelock expires; any caller applies it; future proposals require that PDA's signature.
proof: propose_config.rs:18-26 requires the current governance Signer and its stored key. Its handler stores the supplied change after check_change. config.rs:767-796 rejects zero and repeated authority keys, but accepts any other distinct key. Choose G_bad as an off-curve PDA under an unrelated program that never signs those seeds; keep the valid pause key and fee recipient distinct, and keep coverage disabled. apply_config.rs:61-78 accepts a pending change between not_before and not_before + CONFIG_APPLY_WINDOW_S. config.rs:193 writes G_bad into governance, and apply_config.rs:81 clears the pending change. The application needs the valid writable System-owned fee_recipient, but no account or signature for G_bad. The old governance key then fails propose_config's equality check, while G_bad cannot sign. The source exposes no alternate governance setter outside initialization and the same apply path.
guards: Only current governance may propose. Authorities must be distinct and nonzero, except disabled coverage. The proposal has a timelock and a 14-day application window. The fee recipient must be writable, System-owned, and not executable. None proves control of the successor governance key.
impact: An honest key error can stop all later governance proposals. Pause and coverage key errors also disable their roles until governance replaces them.
limitations: This is an honest-admin hazard, not an outsider takeover. A successful privileged program upgrade can repair the state.
fix: Require each changed, enabled authority to sign the application before the handler replaces its current key.
fix_notes: The existing proposal is the first step; remaining signer accounts provide acceptance at application. The client must include changed authority accounts. The diff is unapplied and uncompiled.
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

LEAD | program: shortwin-venue-state | function: read_base | bug_class: hardening-mint-type-unchecked | group_key: shortwin-venue-state | read_base | hardening-mint-type-unchecked
title: The base mint reader omits account type checks
location: crates/shortwin-venue-state/src/mint.rs:169
code_smells: The reader accepts both token owners and any length of at least 82 bytes; it checks no account type or mint padding.
description: A future caller without a trusted mint key can accept another token account as a mint and use its bytes as supply.
proof: For a 165-byte buffer under TOKEN_PROGRAM, set byte 45 to 1 and leave COption tags at 0 and 46 equal to zero. read_base returns Ok even though SPL mints have exactly 82 bytes. A Token-2022 buffer with the same prefix and account type 2 at offset 165 also passes. The stronger mint::check rejects both shapes. The linked spl-token-2022-interface 2.1.0 StateWithExtensions::<Mint>::unpack checks length, padding, and AccountType::Mint.
path: pump_amm::lp_mint_supply at programs/shortwin/src/venues/pump_amm.rs:419 and keeper::mint_supply at services/keeper/src/view.rs:187 call this weaker reader. PumpSwap's LP key comes from its real pool, and keeper mint keys come from stored markets.
guards: The reader verifies program owner, minimum length, valid COption tags, and initialized byte 1. Listing uses check_mint_safety. Raydium's lp_supply additionally requires Tokenkeg and exactly 82 bytes. Stored market and pool keys prevent caller-selected token-account substitution today.
impact: The library contract permits a wrong account type. A future unbound call could interpret unrelated bytes as fee-tier supply.
limitations: No current unprivileged path to substitute a different account at the trusted mint keys was proved. This remains a hardening lead.
fix: Check legacy mint length and Token-2022 mint padding and account type before reading the base fields.
```diff
--- a/crates/shortwin-venue-state/src/mint.rs
+++ b/crates/shortwin-venue-state/src/mint.rs
@@ -173,6 +173,17 @@
     if data.len() < MINT_LEN {
         return Err(ReadError::TooShort);
     }
+    if data.len() == MULTISIG_LEN || (*owner == TOKEN_PROGRAM && data.len() != MINT_LEN) {
+        return Err(ReadError::WrongTag);
+    }
+    if data.len() > MINT_LEN {
+        let padding = data.get(MINT_LEN..ACCOUNT_TYPE_OFFSET).ok_or(ReadError::TooShort)?;
+        if padding.iter().any(|&b| b != 0)
+            || data.get(ACCOUNT_TYPE_OFFSET) != Some(&ACCOUNT_TYPE_MINT)
+        {
+            return Err(ReadError::WrongTag);
+        }
+    }
     base_of(data)
 }
 
```

LEAD | program: shortwin | function: parse | bug_class: hardening-vault-authority-unchecked | group_key: shortwin | parse | hardening-vault-authority-unchecked
title: The Raydium readers omit vault authority checks
location: programs/shortwin/src/venues/raydium_cpmm.rs:169; programs/shortwin/src/venues/raydium_v4.rs:146
code_smells: Both live parsers verify token programs and mints, but omit the vault authority checks used by their listing verifiers.
description: A future venue change that replaces a vault authority can leave the reader's route executable while the venue rejects every trade.
proof: CPMM parse checks pool-recorded account keys at lines 178-181, then token owners and mints at 186-188. It never compares v0.owner or v1.owner with RAYDIUM_CPMM_AUTHORITY. AMM v4 parse has the same omission at lines 154-162. Both verify functions require t.owner == authority before pool adoption. Live state_of then derives reserves and executable status from the parsed balances without that authority comparison.
guards: Pool ownership and layout are checked. check_read binds the supplied keys to the adopted cache. Token parsing checks program owner, account type, initialization, and COption tags. Initial adoption verifies the expected vault authorities.
impact: The live reader relies on the venue to preserve token authorities after adoption.
limitations: No unprivileged authority-change path was proved against the present venue programs. The initial checks and venue custody prevent a demonstrated exploit today.
fix: Apply the same expected vault authority checks in both live parsers and initial verifiers.
```diff
--- a/programs/shortwin/src/venues/raydium_cpmm.rs
+++ b/programs/shortwin/src/venues/raydium_cpmm.rs
@@ -185,7 +185,10 @@
     let config = amm_config(cfg_i)?;
     let v0 = token_of(v0_i, &Pubkey::new_from_array(pool.token_0_program))?;
     let v1 = token_of(v1_i, &Pubkey::new_from_array(pool.token_1_program))?;
-    if v0.mint != pool.token_0_mint || v1.mint != pool.token_1_mint {
+    if v0.mint != pool.token_0_mint || v1.mint != pool.token_1_mint
+        || v0.owner != RAYDIUM_CPMM_AUTHORITY.to_bytes()
+        || v1.owner != RAYDIUM_CPMM_AUTHORITY.to_bytes()
+    {
         return None;
     }
     Some(Parsed {
--- a/programs/shortwin/src/venues/raydium_v4.rs
+++ b/programs/shortwin/src/venues/raydium_v4.rs
@@ -159,7 +159,10 @@
     }
     let coin = token_of(coin_i, &TOKEN_PROGRAM_ID)?;
     let pc = token_of(pc_i, &TOKEN_PROGRAM_ID)?;
-    if coin.mint != info.coin_vault_mint || pc.mint != info.pc_vault_mint {
+    if coin.mint != info.coin_vault_mint || pc.mint != info.pc_vault_mint
+        || coin.owner != RAYDIUM_V4_AUTHORITY.to_bytes()
+        || pc.owner != RAYDIUM_V4_AUTHORITY.to_bytes()
+    {
         return None;
     }
     Some(Parsed {
```

LEAD | program: shortwin-keeper | function: decode_zc | bug_class: hardening-version-unchecked | group_key: shortwin-keeper | decode_zc | hardening-version-unchecked
title: The keeper reads layouts without their version checks
location: services/keeper/src/view.rs:48
code_smells: The decoder checks owner, discriminator, and size, but never checks the version fields in GlobalConfig, Market, or VenueExtras.
description: After a layout upgrade, the keeper can interpret unsupported account versions and submit transactions that the program rejects.
proof: decode_zc takes the body at bytes 8..8+size_of::<T>() and returns pod_read_unaligned(body). load_with_positions uses it for GlobalConfig at line 266, Market at 289, and VenueExtras at 366 without version checks. A correctly sized GlobalConfig body with version 2 therefore decodes, although CONFIG_VERSION is 1. On-chain ApplyConfig and ProposeConfig require config.version == CONFIG_VERSION. Extras parsing likewise requires VENUE_EXTRAS_VERSION, currently 2.
guards: The keeper requires shortwin ownership, the concrete discriminator, and enough bytes. On-chain handlers reject unsupported config and market versions, so a keeper interpretation cannot bypass those checks.
impact: The keeper may plan from an unsupported layout and repeatedly submit rejected transactions after an incompatible upgrade.
limitations: No current on-chain instruction was found that writes an invalid layout version. This is a future compatibility and hardening gap.
fix: Reject unsupported versions when the keeper decodes versioned protocol accounts.
```diff
--- a/services/keeper/src/view.rs
+++ b/services/keeper/src/view.rs
@@ -51,6 +51,21 @@
     }
     let len = core::mem::size_of::<T>();
     let body = acc.data.get(8..8usize.checked_add(len)?)?;
+    let expected_version = if T::DISCRIMINATOR == GlobalConfig::DISCRIMINATOR {
+        Some(shortwin::constants::CONFIG_VERSION)
+    } else if T::DISCRIMINATOR == Market::DISCRIMINATOR {
+        Some(shortwin::constants::MARKET_VERSION)
+    } else if T::DISCRIMINATOR == VenueExtras::DISCRIMINATOR {
+        Some(shortwin::constants::VENUE_EXTRAS_VERSION)
+    } else {
+        None
+    };
+    if let Some(expected) = expected_version {
+        let version = u16::from_le_bytes(body.get(..2)?.try_into().ok()?);
+        if version != expected {
+            return None;
+        }
+    }
     Some(bytemuck::pod_read_unaligned(body))
 }
 
```
