LEAD | program: common | function: parse_lookup_table | bug_class: hardening-truncated-metadata-accepted | group_key: common | parse_lookup_table | hardening-truncated-metadata-accepted
location: services/common/src/tx.rs:308; common::parse_lookup_table
title: The ALT parser accepts a truncated metadata header.
code_smells: The parser replaces an absent 56-byte metadata header with an empty address list.
description: The keeper accepts truncated ALT data; a malformed RPC response or future loader path can hide the failure as an empty table.
proof: With owner = ALT_PROGRAM_ID and data = [1, 0, 0, 0], the tag equals Some(1); data.get(56..) is None; unwrap_or(&[]) yields an empty body; 0 % 32 == 0; the function returns Ok(AddressLookupTableAccount { key, addresses: [] }). A real initialized ALT requires its metadata header. This pass did not establish that an unprivileged caller can create such truncated ALT-owned chain state. The current security impact remains unverified.
fix: Reject a missing metadata header before the parser reads any address.
proposed_diff:
```diff
--- a/services/common/src/tx.rs
+++ b/services/common/src/tx.rs
@@ -305,7 +305,7 @@
     if tag != Some(1) {
         bail!("{key} is not an initialized lookup table");
     }
-    let body = data.get(ALT_META_SIZE..).unwrap_or(&[]);
+    let body = data.get(ALT_META_SIZE..).context("truncated lookup table metadata")?;
     if body.len() % 32 != 0 {
         bail!("{key}: truncated lookup table");
     }
```

LEAD | program: keeper | function: load_with_positions | bug_class: hardening-version-unchecked | group_key: keeper | load_with_positions | hardening-version-unchecked
location: services/keeper/src/view.rs:263; services/keeper/src/view.rs:284; keeper::load_with_positions
title: The keeper loads account layouts without their version checks.
code_smells: decode_zc checks the owner, discriminator, and minimum byte length; load_with_positions accepts GlobalConfig and Market without a version check.
description: The keeper interprets a future account layout with its old types if an upgrade preserves the discriminator and minimum size.
proof: view.rs:48-55 uses pod_read_unaligned after the owner/discriminator/size checks. load_with_positions:263-267 accepts GlobalConfig, and its market loop accepts decoded Market data before venue_kind; neither compares CONFIG_VERSION or MARKET_VERSION. The on-chain open_short Accounts constraints require both versions. A stored version byte different from the expected version therefore reaches keeper projection while the on-chain instruction refuses it. No currently reachable unauthorized state write or direct fund loss was proved.
fix: Check each decoded config and market version before the keeper uses its fields.

LEAD | program: shortwin | function: inputs | bug_class: transaction-controlled-settlement-legs | group_key: shortwin | inputs | transaction-controlled-settlement-legs
location: programs/shortwin/src/engine/arming.rs:208; programs/shortwin/src/engine/arming.rs:213; shortwin::inputs
title: A temporary withdrawal changes the live settlement legs.
code_smells: The settlement input uses live venue reserves for b_live and sigma_live, while only the CR gate uses the EMA liquidity floor.
description: An LP can force SOL settlement after a temporary withdrawal; the current borrower loss and profitability still need an independent numerical trace.
path: LP withdraws unlocked venue liquidity -> sol_settle reads the reduced live reserves -> LP restores liquidity in the same transaction; repeat across the required observation updates.
proof: inputs:208 reads state.liquidity(); inputs:213-216 computes quotes.live(state.x, state.y); inputs:223 applies bounds::spot_liquidity only to the CR quote; SettleInputs carries the original l_live, b_live, and sigma_live. The module comment at arming.rs:40-52 states that AIR-20 remains reachable when the honest state lies outside the box, and identifies a 20% pool_lock_min_bps / 1% fee case. That prior disclosure is context, not a guard. This pass traced the adapter/transaction boundary but did not independently reconstruct current box corners, arming state, borrower payout, and attacker financing with concrete numbers. No finding is asserted from that comment alone.
fix: Require a live settlement leg that a temporary liquidity withdrawal cannot create.

LEAD | program: shortwin | function: assess | bug_class: manipulable-liquidity-history | group_key: shortwin | assess | manipulable-liquidity-history
location: programs/shortwin/src/instructions/liquidate.rs:356; shortwin::assess
title: Repeated withdrawals can preserve an earlier low liquidity estimate.
code_smells: The liquidation spot floor trusts the EMA liquidity history; the source states that repeated temporary withdrawals can preserve an earlier low estimate.
description: An LP can preserve an earlier low liquidity estimate with repeated withdrawals; the current forced-liquidation payoff still needs an independent numerical trace.
path: LP holds liquidity outside the venue until the EMA falls -> LP restores liquidity -> LP wraps observations and the eventual liquidation with withdrawals and redeposits.
proof: assess:356-361 computes l_spot = spot_liquidity(l_live, obs.ema, obs.ring) and then prices b_rs at l_spot. The liquidate module comment at lines 35-43 explicitly names FITX-1 as a retained held-withdrawal/frozen-history path. Search-only context found services/keeper-litesvm/tests/scenarios/freeze.rs and tests/litesvm/tests/pamm_it/lock.rs describing cases, but this pass did not read their complete scenario bodies or execute them. The retained disclosure does not refute the candidate; current profitability and the full observation sequence remain unverified.
fix: Reject liquidation evidence that temporary withdrawals preserve after the venue regains liquidity.
