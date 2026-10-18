FINDING | program: shortwin-math | function: withdraw_in | bug_class: carry-crosses-empty-pool | group_key: shortwin-math | withdraw_in | carry-crosses-empty-pool
title: The last withdrawal leaves carry for the next lender
location: crates/shortwin-math/src/pool.rs:1907-1912; related lines 1033-1042, 1125-1136, 1997-2053, 2100-2137, 2501-2505
seam: execution×first-principles
root_cause: The last-share withdrawal clears sweep flags but retains both index remainders for a later lender population.
description: A new lender receives the previous lenders' remaining SOL because the last withdrawal preserves both index remainders.
trace: deposit → external token transfer → sweep of real SOL → final withdrawal and lender close → empty-pool sweep → new deposit → new SOL credit → claim.
violated_principle: SOL earned before every lender exits must not become the next depositor's entitlement.
path: Use a valid Raydium market with no pump UVA, no open positions, no prior settlement hold, and sufficient safe-mint supply. Follow the full sequence below.
proof: The following integer trace starts from Pool::new and uses actual instruction paths. WAD=10^18, VS=1000, VA=1. (1) Lender A deposits 3,000,000,000,000,000 raw tokens. The token CPI succeeds and S=3,000,000,000,000,000,000. (2) Two lamports reach the trader from ordinary third-party income or a donation. sweep_trader transfers them to MarketVault and pool::sweep vests them, with lender_sol=2. (3) A withdraws every share with min_tokens equal to the deposited tokens and min_sol=0. The last-share branch releases all remaining vest. credit_pos and credit_r_in each compute floor(2*10^18/(3*10^18))=0 and carry=carry_r=2*10^18. A receives every deposited token; A's SOL pending and real stay 0. (4) withdraw_in sets S=0 and wipe_swept=0, but leaves both carries. The program's LastSharesBorrowed guard passes because borrowed_q=0. pool::closable is true; withdraw closes A's lender position. (5) sweep_trader runs with zero lamports. This sets WIPE_TRADER_SWEPT, so the next deposit passes the empty-pool sweep requirement. No reward bit is required on this market. (6) B deposits one raw token and receives 1000 shares. Lender::new snapshots acc=acc_r=0; it neither clears nor snapshots away carry. (7) B sends one lamport to the trader and calls sweep_trader. B then withdraws every share, which flushes this one-lamport vest. Both index updates compute (10^18+2*10^18)/1000=3*10^15 with no remainder. B's touch assigns floor(1000*3*10^15/10^18)=3 lamports to pending and real. claim pays all three backed lamports. B supplied one and received two from A's previous pool population.
guards: Owner signatures authorize deposits, withdrawals, and claims. PDA bindings and token transfers remain valid. There are no borrowed tokens, locks, epoch conversions, pending reward claims, or settlement holds. The zero-value sweep satisfies the actual reset gate. Both token postconditions and the MarketVault rent floor pass. The trace needs an old pool share count greater than WAD; deposit accepts these u64 token amounts and u128 shares.
impact: The next depositor captures old SOL remainder. The amount depends on the previous share count and both carries; this is not an arbitrary vault withdrawal.
limitations: This is static integer proof, not a transaction reproduction. The example requires a valid mint with the stated available raw supply. It does not prove a profitable attack on each listed market.
fix: Move both remainders to dust and clear them when the last withdrawal takes total_shares to zero.
minimal_diff:
```diff
--- a/crates/shortwin-math/src/pool.rs
+++ b/crates/shortwin-math/src/pool.rs
@@ -1908,6 +1908,9 @@
     if p.total_shares == 0 {
         // AIR-08: the market has no lender again; SOL that reaches the trader from now on is
         // the protocol's (KI-10), so the next deposit waits for the sweeps, as after a wipe.
+        p.dust = add_i128(add_i128(p.dust, p.carry)?, to_i128(p.carry_r)?)?;
+        p.carry = 0;
+        p.carry_r = 0;
         p.wipe_swept = 0;
     }
     let amount = to_u64(amount)?;
```
validation: Proposed diff remains unapplied and uncompiled. The reset follows the existing maybe_roll_in dust treatment at pool.rs:2501-2505.

LEAD | program: shortwin | function: deposit | bug_class: donation-extends-deposit-hold | group_key: shortwin | deposit | donation-extends-deposit-hold
title: Small donations can renew a settlement hold
location: programs/shortwin/src/instructions/deposit.rs:130-135; programs/shortwin/src/instructions/sol_settle.rs:325-328; programs/shortwin/src/instructions/sweep_trader.rs:275-284, 306-386; crates/shortwin-math/src/pool.rs:898-1003
seam: execution×periphery×first-principles
code_smells: The deposit hold follows the entire shared vest, including later unsolicited SOL.
description: An outsider can renew a settlement hold with a small donation; the review does not establish a guaranteed long-term block.
trace: A settlement sets deposit_hold_until and leaves S>0 → original vest reaches its end → outsider funds trader and calls sweep_trader → a fresh vest prevents deposit.
proof: deposit checks only whether the marker is nonnegative, then requires pool.vest_bal==0. It does not clear the marker merely because its recorded timestamp passed. At the original vest end, sweep_trader's prologue releases the original balance; its subsequent sweep of one lamport starts a fresh vest. The default lump_vest_s is 21,600 seconds. The marker remains nonnegative because only a successful deposit resets it. A deposit in that interval fails PoolDrained even though the original settlement vest completed. The same sequence can repeat at the next end if the outsider lands before a deposit. On an already-running vest, floor rounding limits small donations: the review does not claim that each lamport always extends the end.
guards: The market must already have a hold and surviving shares. The sweep must satisfy its stored trader keys and applicable token-leg accounts. An ordinary market with deposit_hold_until=-1 does not acquire this hold from a simple donation. A final lender withdrawal can flush the vest; a deposit that lands first at release can clear the marker.
limitations: The source explicitly requires later legitimate credits to remain protected until the shared vest empties. This lead concerns unsolicited donations and timing around the end, not an incorrect timestamp comparison. A sustained block and its economic impact remain unverified.
fix_direction: Separate the amount protected by a settlement hold from later unsolicited donations while preserving the old holders' pending cashback.

LEAD | program: shortwin | function: sync_lender | bug_class: repeated-accrual-truncation | group_key: shortwin | sync_lender | repeated-accrual-truncation
title: Repeated sync calls discard fractional lender credits
location: programs/shortwin/src/instructions/sync_lender.rs:118-138; crates/shortwin-math/src/pool.rs:598-600, 1459-1502, 1660-1690
seam: execution×first-principles
code_smells: Each current-epoch touch floors the interval credit and advances its snapshots without a fractional remainder.
description: An outsider can discard another lender's fractional credits with repeated sync calls; the review does not establish an economical attack.
trace: Shared SOL index advances → outsider calls sync_lender(0) for a current lender → touch_pot reaches touch_tail → snapshots advance after a floored credit.
proof: A current lender with 1000 shares and no forward chain receives floor(1000*delta/10^18). For two successive index increments of 5*10^14, a touch after each increment credits 0+0 lamports; one touch after both credits 1. Both pending and real use the floored interval product. touch_tail then assigns snap=pool.acc and snap_r=pool.acc_r, so the lost fractions cannot reach a later claim. A pool with 2000 shares obtains each increment from one released lamport. The owner need not sign: sync_lender intentionally permits the current-epoch touch through touch_pot. The rebase restriction and unchanged-record postcondition both pass because no conversion occurs.
guards: A second sync at an unchanged index loses nothing. The index must advance between touches. The owner and market keys still match the lender PDA. This lead does not bypass the owner-only rebase conversion.
limitations: The trace demonstrates frequency-dependent accounting. It does not establish profitable extraction, large losses per call, or a particular live pool's index cadence. The forward-chain branch uses cumulative floors and is not the branch identified here.
fix_direction: Retain per-lender fractional credits or use a cumulative accrual calculation that remains correct across share changes.

LEAD | program: shortwin | function: sweep_trader | bug_class: external-cashback-recovery-dependency | group_key: shortwin | sweep_trader | external-cashback-recovery-dependency
title: Cashback cleanup depends on external claim instructions
location: programs/shortwin/src/instructions/sweep_trader.rs:129-135, 248-263, 334-350, 371-386; programs/shortwin/src/engine/lender.rs:202-208; programs/shortwin/src/venues/pump_amm.rs:791-803; crates/shortwin-math/src/pool.rs:2025-2035
seam: periphery×first-principles
code_smells: The reward-clear bit depends on external cashback accounts that this sweep cannot empty.
description: A failed external cashback claim can keep deposits blocked after a wipe; this review does not prove a current failure.
trace: A wipe or last-share withdrawal clears sweep flags → sweep_trader finds outstanding cashback → the reward bit stays clear → deposit fails WipeSweepPending.
proof: rewards_required returns true for a stored pump UVA or any pump venue. sweep_trader requires the pump UVA's excess lamports to be zero and, on PumpSwap, its cashback WSOL ATA to be empty. The sweep reads those accounts but does not transfer their balances. Its own trader cleanup therefore cannot set WIPE_REWARDS_CLAIMED while external cashback remains. The documented recovery calls pump or PumpSwap claim_cashback before sweep_trader. A program failure that prevents that external claim leaves no clearing path in the reviewed short.win instructions.
guards: Valid external claims are permissionless and can be bundled with the sweep. Missing external accounts normally mean no cashback, so a missing empty account alone does not prove a block. Existing balances remain assigned to the prior lenders; bypassing the bit would transfer their value to later holders. This issue concerns dependence on an unavailable claim, not missing signer authorization.
limitations: No external claim failure or reachable malicious external state was verified. The source comments state that deployed claim paths work; their cited runtime tests were not rerun. This remains a dependency lead, not a proven deposit outage.
fix_direction: Document and validate recovery when the external claim interface changes; preserve the prior holders' cashback entitlement in any fallback.
