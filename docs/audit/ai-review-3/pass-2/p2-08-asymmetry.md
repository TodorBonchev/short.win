FINDING | program: shortwin | function: apply_config | bug_class: unsafe-authority-transfer | group_key: shortwin | apply_config | unsafe-authority-transfer
title: Authority changes accept a successor without its signature
file: programs/shortwin/src/instructions/apply_config.rs:31,57-82; programs/shortwin/src/state/config.rs:187-196,769-799
pair_or_branch: propose_config / apply_config; current-governance authorization / successor-governance authorization
asymmetry: propose_config proves that the current governance key can sign; apply_config writes the replacement governance key without its signature.
root_cause: The Authorities branch accepts a nondefault, distinct successor pubkey without proof that the successor can authorize later proposals.
path: Honest governance G proposes Authorities with mistaken successor G_bad; the timelock expires; any caller applies it; config.governance becomes G_bad.
proof: propose_config.rs:19,25 requires G as a Signer and checks it against config.governance. Its handler validates the proposed shape and stores the pending payload. config.rs:769-799 only requires nondefault keys and distinct roles. A nondefault G_bad that differs from the other three roles passes. apply_config.rs:58-82 checks the pending change, not_before, the apply window and the shape, then applies and clears it. GlobalConfig::apply assigns self.governance = *governance at config.rs:193. ApplyConfig has only config and optional fee-recipient/oracle accounts; it has no successor signer. If nobody controls G_bad, G can no longer pass propose_config.rs:25. The Emergency role cannot propose an Authorities correction; cancel_config.rs:40-44 also reserves cancellation of Authorities to governance. After application there is no pending proposal left to cancel.
guards: The current governance must authorize the proposal. The configured timelock and 14-day apply window must pass. Keys must be nondefault and distinct. The new fee recipient must match its supplied System-owned, nonexecutable account. None proves control of G_bad.
impact: An honest mistaken authority change disables all later governance proposals, including parameter updates and authority recovery, through the current instruction set.
limitations: An outsider cannot create the mistaken proposal. A retained program upgrade authority may restore access through a program upgrade. The finding does not claim an outsider can steal funds.
description: An honest governance proposal can install an unusable successor and block all later configuration changes because the successor never signs.
fix: Require the replacement governance authority to sign before apply_config installs its key.
minimal_diff: The following proposed patch is unapplied and uncompiled; clients must supply the added optional account.
```diff
--- a/programs/shortwin/src/instructions/apply_config.rs
+++ b/programs/shortwin/src/instructions/apply_config.rs
@@ -52,6 +52,8 @@
         bump = oracle_cache.bump
     )]
     pub oracle_cache: Option<Box<Account<'info, OracleCache>>>,
+    /// Required when an Authorities change replaces governance.
+    pub new_governance: Option<Signer<'info>>,
 }
 
 pub fn handler(ctx: Context<ApplyConfig>) -> Result<()> {
@@ -76,6 +78,13 @@
         );
         let change = c.pending.change()?;
         c.check_change(&change)?;
+        if let ConfigChange::Authorities { governance, .. } = &change {
+            if *governance != c.governance {
+                let successor = ctx.accounts.new_governance.as_ref()
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
file: programs/shortwin/src/instructions/close_short.rs:83-91; programs/shortwin/src/instructions/liquidate.rs:121-129; programs/shortwin/src/instructions/expire.rs:82-90; programs/shortwin/src/instructions/sol_settle.rs:122-128
pair_or_branch: add_margin / close_short / liquidate / expire / sol_settle
asymmetry: add_margin requires position.status == POSITION_OPEN; all four exit contexts omit this predicate.
code_smells: The exits check the position PDA, borrower and market, but they do not check its status.
proof: add_margin.rs:65 rejects any status except POSITION_OPEN. position.rs:19-21 defines Open as 0 and reserves ExitInProgress as 1. Each listed exit context accepts the same ShortPosition type without a status constraint. Searches across production programs/shortwin/src found only open_short.rs:616 assigning status, and it assigns POSITION_OPEN. The current exit handlers close the account when execution completes; sol_settle retains only the arming state when execution does not complete.
guards: Account ownership and discriminator, the stored-bump position PDA, borrower/market bindings, and instruction-specific eligibility and payment bounds remain checked. No current production path creates status 1.
impact: A future path that writes ExitInProgress could leave a position eligible for a second exit path unless all exit contexts add the state check.
limitations: This is a hardening gap. No presently reachable non-Open position or double payout was established.
description: The exits omit the status check that add_margin uses, so a future pending-exit state could permit an extra exit.
fix: Require POSITION_OPEN in each exit context before the handler processes the position.

LEAD | program: shortwin | function: deposit | bug_class: hardening-pool-vault-fields-unchecked | group_key: shortwin | deposit | hardening-pool-vault-fields-unchecked
title: Pool vault loads omit token field constraints
file: programs/shortwin/src/instructions/deposit.rs:85-99; programs/shortwin/src/instructions/withdraw.rs:118-132; programs/shortwin/src/instructions/open_short.rs:164-171; programs/shortwin/src/instructions/close_short.rs:100-107; programs/shortwin/src/instructions/liquidate.rs:138-145; programs/shortwin/src/instructions/expire.rs:99-106
pair_or_branch: owner_token_account / pool_vault constraints; create_pool_vault / later vault loads
asymmetry: The owner token account binds its mint and token program explicitly. The pool vault binds its address but omits equivalent token fields and authority checks.
code_smells: The pool vault contexts lack token::mint, token::authority and token::token_program constraints.
proof: deposit.rs:88-93 and withdraw.rs:121-126 check the owner's token-account mint and token program. Their pool_vault declarations use InterfaceAccount<TokenAccount>, its stored-bump PDA and market.has_one, but do not bind its decoded mint, authority or selected token program. The open and three buyback exits repeat the same declaration. create_market.rs:281-350 creates this exact PDA under the selected token program and initializes it with accs.mint and accs.market as authority. Deposit's transfer_checked and subsequent amount check do not independently restate all three vault fields. Withdraw's signed transfer also relies on the existing vault state.
guards: The vault address is fixed by its PDA and market.has_one. create_pool_vault assigns the intended program, mint and authority. The token program enforces transfer rules. The current program exposes no reviewed path that changes this vault's authority or replaces its mint.
impact: A future migration or authority-changing path could violate the vault assumptions while these loaders still accept its address.
limitations: This is a hardening gap. The reviewed creation path prevents callers from substituting an arbitrary token account today.
description: The pool vault loaders omit explicit token field checks, so a future vault mutation could violate assumptions that creation currently enforces.
fix: Add the expected mint, market authority and selected token program constraints to each pool vault declaration.

LEAD | program: shortwin | function: deposit | bug_class: donation-extends-deposit-hold | group_key: shortwin | deposit | donation-extends-deposit-hold
title: Small donations can extend a settlement hold
file: programs/shortwin/src/instructions/deposit.rs:130-135; programs/shortwin/src/instructions/sol_settle.rs:322-329; programs/shortwin/src/instructions/sweep_trader.rs:184,282-291,358-370; crates/shortwin-math/src/pool.rs:898-928,948-988,2200-2229
pair_or_branch: settlement-created deposit hold / unrelated trader donation; fixed stored marker / live shared vest
asymmetry: sol_settle records a hold from settlement vesting; deposit waits for the entire current vest balance, including later trader donations.
code_smells: The deposit guard uses deposit_hold_until only as a flag and blocks on pool.vest_bal instead of a settlement-specific balance.
proof: sol_settle.rs:322-329 can leave deposit_hold_until nonnegative after a nonflushed settlement. deposit.rs:130-135 clears that marker only when an attempted deposit observes vest_bal == 0. SweepTrader has no signer requirement and normalises donated lamports before pool::sweep. engine/trader.rs:142-183 moves the trader's full lamport balance into market_vault. pool.rs:2200-2229 passes ordinary trader sweeps to vest_in when shares remain and the source has no after-flush immediate-credit entitlement. Suppose a held market's old vest ends at t0, no deposit has yet cleared its marker, and shares remain. A donation followed by sweep_trader at t0 first releases the old vest, then creates a new vest. With one donated lamport and lump_vest_s = 21600, vest_in sets vest_bal = 1 and vest_end = t0 + 21600. A deposit at t0 + 1 computes floor(1 / 21600) = 0 in release_in, leaves vest_bal = 1 and returns PoolDrained. The stale hold marker therefore blocks deposits after the original settlement vest ends.
guards: The market must already have an active hold, must retain shares, and must not have cleared the marker through a successful deposit. The after-flush immediate-credit branch must not apply. The value-weighted formula prevents arbitrary end extensions from dust while a large vest remains. Withdrawals and claims remain available.
impact: A caller may extend the period during which new lenders cannot deposit through a small unrelated donation.
limitations: This remains an economic and liveness lead. The review did not establish an attacker profit model or reproduce the exact low-value donation transaction under runtime rent checks. The source explicitly protects later extensions of the shared vest, but does not state whether repeated unrelated donations should extend the admission hold.
description: A donor may extend a settlement hold through unrelated vesting; the review leaves the intended admission policy and exact donation cost unverified.
fix: Track the protected settlement vest separately so unrelated donations cannot prolong its deposit hold.
