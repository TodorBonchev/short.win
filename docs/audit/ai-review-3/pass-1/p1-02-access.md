FINDING | program: shortwin | function: apply_config | bug_class: unsafe-authority-transfer | group_key: shortwin | apply_config | unsafe-authority-transfer
title: Authority changes install governance keys without signed acceptance
file: programs/shortwin/src/instructions/apply_config.rs:80
related_locations: programs/shortwin/src/state/config.rs:193; programs/shortwin/src/state/config.rs:769; programs/shortwin/src/instructions/propose_config.rs:19
severity: medium
confidence: high
root_cause: apply_config installs a different governance key from the approved payload without proof that the successor can sign.
guard_gap: ProposeConfig requires the current governance as Signer and compares its key at propose_config.rs:19-25; ApplyConfig has no successor Signer at apply_config.rs:35-55. check_authorities rejects default and duplicate keys, but accepts an unreachable nondefault key.
path: An honest old governance signer proposes Authorities with an unusable successor -> an independent caller waits for not_before -> apply_config commits the successor and clears pending -> no caller can authorize another configuration proposal.
proof: Let C be the existing canonical config PDA, G the live governance signer, P the live pause authority, F a valid System-owned fee recipient, and coverage_authority = Pubkey::default(). Let C, G, P, and F differ. With otherwise valid parameters and timelock_s = 172800, G submits propose_config(Authorities { governance: C, pause_authority: P, coverage_authority: default, fee_recipient: F }) at t0 = 2000000000. ProposeConfig authenticates G; check_authorities checks only nondefault values and pairwise distinctness (config.rs:775-795), so C passes. The proposal stores not_before = 2000172800 (propose_config.rs:34-39). At that second, an unrelated fee payer submits apply_config with C writable, F writable, oracle_cache omitted, and the required event accounts. No governance account or governance signature is required. The time bounds and unchanged valid parameter set pass. GlobalConfig::apply writes self.governance = C (config.rs:193), and apply_config clears pending (apply_config.rs:80-81). F passes the recipient check. C is an off-curve PDA. The source contains no CPI signer seeds for GlobalConfig::SEED and no instruction that restores governance outside initialization or this same proposal flow. Every future propose_config now needs C as a Signer (propose_config.rs:19-25); the old G fails the key check and an unsigned C fails Anchor's signer check. The existing program cannot recover this role. Its upgrade authority can recover only through a program upgrade. The 48-hour delay permits cancellation if humans detect the mistake; it does not prove successor control. Anchor 1.2.0 signer.rs:49-56 explicitly checks info.is_signer. No runtime PoC was run.
description: An honest admin can propose an unusable successor, and anyone can apply it after the timelock, so configuration changes then require a program upgrade.
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

LEAD | program: shortwin | function: close_short | bug_class: hardening-position-status-unchecked | group_key: shortwin | close_short | hardening-position-status-unchecked
title: Exit instructions omit the position status check
file: programs/shortwin/src/instructions/close_short.rs:82
related_locations: programs/shortwin/src/instructions/liquidate.rs:123; programs/shortwin/src/instructions/expire.rs:93; programs/shortwin/src/instructions/sol_settle.rs:122; programs/shortwin/src/instructions/add_margin.rs:63; programs/shortwin/src/state/position.rs:18
code_smells: AddMargin requires position.status == POSITION_OPEN, but CloseShort, Liquidate, Expire, and SolSettle do not.
description: A future instruction that sets POSITION_EXIT_IN_PROGRESS can leave its position eligible for close_short and keeper exits because those paths omit status checks.
limits: This is not an exploit in the reviewed build. POSITION_EXIT_IN_PROGRESS is reserved for v1.1, and open_short currently writes only POSITION_OPEN. The independent gate already exists in add_margin; the other four Accounts structs check the account's borrower, market, and PDA but not its status. A future non-open state must be excluded explicitly before this omission becomes reachable.
fix: Add the POSITION_OPEN constraint to each exit instruction before any instruction writes another live position status.
