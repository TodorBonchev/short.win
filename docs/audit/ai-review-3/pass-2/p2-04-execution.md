LEAD | program: shortwin | function: close_short | bug_class: hardening-position-status-unchecked | group_key: shortwin | close_short | hardening-position-status-unchecked
file: programs/shortwin/src/instructions/close_short.rs:84
related_files: programs/shortwin/src/instructions/liquidate.rs:122; programs/shortwin/src/instructions/expire.rs:83; programs/shortwin/src/instructions/sol_settle.rs:122; programs/shortwin/src/instructions/add_margin.rs:65; programs/shortwin/src/state/position.rs:19
location: shortwin::close_short
title: Exit handlers omit the position status check
code_smells: The four exit handlers check the position address and relationships but omit its status; add_margin checks POSITION_OPEN.
description: A future migration can expose non-open positions to exit handlers because those handlers do not check the status.
input: A caller supplies a genuine program-owned ShortPosition with a future status other than POSITION_OPEN, plus its correct market, borrower, and exposure.
assumption: Every correctly addressed ShortPosition that reaches an exit remains open.
path: A future instruction writes POSITION_EXIT_IN_PROGRESS -> close_short, liquidate, expire, or sol_settle loads the genuine position -> the handler applies its normal exit accounting.
proof: ShortPosition defines POSITION_OPEN=0 and reserves POSITION_EXIT_IN_PROGRESS=1 at state/position.rs:19-21. The four exit account structs check has_one and position.is_pda but never position.status. Their handlers copy the position and proceed directly to settlement or buyback calculations. add_margin.rs:65 demonstrates the missing defensive comparison. A cross-file search found open_short.rs:616 as the only production status assignment, and it writes POSITION_OPEN.
guards: Anchor checks the account owner and discriminator. The position PDA, market, borrower, and exposure bindings prevent a caller from fabricating the future status. All financial guards still apply. The current program never writes the reserved status. Anchor 1.2.0 closes an account by assigning System ownership and resizing to zero; its account exit skips a closed account, so this is not a current account-revival path.
impact_and_limitations: This is a future-state hardening gap. No current attacker can create the required position state through the reviewed instructions. No current double close, double payout, or loss is proved.
fix: Require POSITION_OPEN in each exit account struct before the handler uses the position.
proposed_diff: Unapplied and uncompiled; this adds the same defensive check to each affected exit.
```diff
--- a/programs/shortwin/src/instructions/close_short.rs
+++ b/programs/shortwin/src/instructions/close_short.rs
@@ -89,1 +89,2 @@
-        constraint = position.is_pda(&position.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds
+        constraint = position.is_pda(&position.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds,
+        constraint = position.status == crate::state::POSITION_OPEN @ ShortwinError::AccountMismatch
--- a/programs/shortwin/src/instructions/liquidate.rs
+++ b/programs/shortwin/src/instructions/liquidate.rs
@@ -127,1 +127,2 @@
-        constraint = position.is_pda(&position.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds
+        constraint = position.is_pda(&position.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds,
+        constraint = position.status == crate::state::POSITION_OPEN @ ShortwinError::AccountMismatch
--- a/programs/shortwin/src/instructions/expire.rs
+++ b/programs/shortwin/src/instructions/expire.rs
@@ -88,1 +88,2 @@
-        constraint = position.is_pda(&position.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds
+        constraint = position.is_pda(&position.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds,
+        constraint = position.status == crate::state::POSITION_OPEN @ ShortwinError::AccountMismatch
--- a/programs/shortwin/src/instructions/sol_settle.rs
+++ b/programs/shortwin/src/instructions/sol_settle.rs
@@ -126,1 +126,2 @@
-        constraint = position.is_pda(&position.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds
+        constraint = position.is_pda(&position.key()) @ anchor_lang::error::ErrorCode::ConstraintSeeds,
+        constraint = position.status == crate::state::POSITION_OPEN @ ShortwinError::AccountMismatch
```

LEAD | program: shortwin | function: inputs | bug_class: transaction-controlled-settlement-legs | group_key: shortwin | inputs | transaction-controlled-settlement-legs
file: programs/shortwin/src/engine/arming.rs:208
related_files: programs/shortwin/src/engine/arming.rs:223; programs/shortwin/src/engine/arming.rs:259; crates/shortwin-math/src/settle.rs:156; crates/shortwin-math/src/settle.rs:173; crates/shortwin-math/src/settle.rs:195; crates/shortwin-math/src/settle.rs:615
location: shortwin::inputs
title: Temporary withdrawals change the live settlement legs
code_smells: The spot gate uses the EMA liquidity floor, but the S1 and S2 legs use live liquidity from the caller's transaction.
description: An LP can change settlement legs through temporary withdrawals; borrower loss outside the documented residual remains unproved.
input: An unlocked LP supplies a valid sol_settle instruction between its venue withdrawal and redeposit. The position is already expired or passes both liquidation gates.
assumption: The box corner rejects harmful changes to the live settlement legs even when the honest live state lies outside the box.
path: Venue withdrawal -> sol_settle reads reduced reserves -> inputs retains reduced l_live and the reduced-reserve live quote -> S1/S2 evaluates those values and the unchanged box corner -> the transaction restores liquidity after the settlement call.
proof: inputs reads state.liquidity at arming.rs:208 and computes b_live and sigma_live from state.x/state.y at lines 214-220. It computes the protected l_spot separately at line 223. SettleInputs still receives the original l_live at line 259. settle::s1_legs checks both b_live and quotes.cost(p_rs,l_live), and s2_legs checks sigma_live and quotes.sigma(p_rs,l_live). The refutation uses min(l_live,bx.l_lo), so a withdrawal cannot help refute. These reads occur before any venue CPI; they reflect all earlier instructions in the same transaction. This independently confirms control of those inputs, not a profitable attack.
conditional_arithmetic: At price 1, an exact-out fee-free quote for q=100 from reserves (1000,1000) costs ceil(1000*100/900)=112. A proportional withdrawal to (200,200) raises it to 200. If the box's least-favorable S1 corner remains (P=1,L=200), ert=0, and V-K_s=150, all three S1 legs hold only during the withdrawal. This illustrates the predicate change. It does not establish that this complete account state satisfies the protocol's open caps or that the state persists for the required updates.
guards: The CR spot gate floors liquidity at spot_liquidity, and the EMA leg also applies. The expired alternative still applies. Warm-up, a fresh robust spot, the independent box corner, the arming gap, restart_ema_count, and settle_persist_n remain required. Repeated calls in one second cannot create the required EMA updates. S2 and S3 also require the box amount check. Failed later instructions revert the settlement and withdrawal together.
impact_and_limitations: An LP can affect arming or settlement only if the independent box corner already permits it and the honest live state lies outside that box. arming.rs:41-54 explicitly documents this AIR-20 residual, including the reported absence of borrower loss at default parameters. This pass establishes no new exploit outside that stated residual and does not promote the lead to a finding. No runtime reproduction was performed.
fix: Compare reachable borrower payouts before any change to the documented live-liquidity leg rules.
remediation_limit: No code diff is proposed because an EMA floor on these legs changes the stated economic design and this pass proves no new loss.
