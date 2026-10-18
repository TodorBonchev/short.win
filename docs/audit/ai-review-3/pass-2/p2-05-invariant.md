FINDING | program: shortwin-math | function: withdraw_in | bug_class: carry-crosses-empty-pool | group_key: shortwin-math | withdraw_in | carry-crosses-empty-pool
title: The last withdrawal leaves carry for the next lender
location: crates/shortwin-math/src/pool.rs:1906-1912
related_locations: crates/shortwin-math/src/pool.rs:1033-1044,1125-1137,1997-2046,2100-2139,2200-2230,2499-2505; programs/shortwin/src/instructions/withdraw.rs:194-224; programs/shortwin/src/instructions/deposit.rs:130-154; programs/shortwin/src/instructions/sweep_trader.rs:310-397
root_cause: withdraw_in clears only wipe_swept when the final burn reduces total_shares to zero; it leaves both distribution carries for subsequent shares.
description: A new lender collects SOL left in the previous pool's carries after the last lender exits.
invariant: SOL distribution remainders accumulated for departed shares must not become claims of a later lender after the pool has no shares.
violation_path: An old lender deposits tokens; a trader sweep credits a lump; time releases the lump; the old lender withdraws every share; a zero trader sweep clears the deposit gate; a new lender deposits one token; a later one-lamport credit allocates both old carries to that new lender.
path: Use a safe, fixed-supply SPL mint on a list-only market without a pump UVA. All accounts are the canonical market, lender, vault, trader, token-program and owner accounts. The old lender signs its deposit and withdrawal. The new lender signs its own deposit and claim; anyone can send sweep_trader.
proof: Start with T=10^18 raw tokens, B=0, S=1000*T=10^21, acc=acc_r=carry=carry_r=0, and one old lender. Deposit has no total-token cap below this u64 amount. Donate 500 lamports to the trader and call sweep_trader in the same transaction. At the configured vest end, a market-writing call releases 500 lamports. Both divisions use floor(500*10^18/10^21)=0, leaving carry=carry_r=500*10^18 and lender_sol=500. The old lender then withdraws S shares. Its token payout is floor(S*(T+1)/(S+1000))=T, and its pending/real remain zero. The burn leaves S=T=B=0, both carries unchanged, and lender_sol=500. The owner gets all deposited tokens and the empty lender account can close. A zero sweep sets WIPE_TRADER_SWEPT without changing the carries. A new deposit of one raw token mints 1000 shares with snap=snap_r=0. Donate one lamport to the trader, sweep it, and wait for its vest release. Each accumulator gains floor((1*10^18+500*10^18)/1000)=501000000000000000. The new lender now receives floor(1000*501000000000000000/10^18)=501 lamports through claim_sol. Only one lamport entered after the new deposit; the other 500 came from the old distribution. This trace uses pure integer calculations and source inspection, not a runtime reproduction.
guards: LastSharesBorrowed passes because B=0 throughout. The drained-pool guard passes because B=0. No live wipe record exists. A zero trader sweep satisfies the empty-pool gate on this non-pump market. The old vest reaches zero before the new deposit, so any initialized deposit hold passes. The lamports remain physically present, so claim_sol's lender_sol cap and the rent-plus-tracked vault floor also pass. No signer, PDA, mint or token-program substitution is required.
impact: A later depositor captures the previous pool's undistributed SOL. The amount can exceed one lamport, as the 500-lamport example shows. Per-lender fractional accrual loss is a separate issue.
limitations: The example needs a mint with 10^18 raw units available to the old lender. It demonstrates misallocation of existing SOL, not creation of unbacked SOL. New external value must trigger credit_pos and credit_r_in after the new deposit; the attacker can supply one lamport through the documented sweep path.
fix: Clear both carries when the last shares exit, and book them as dust as maybe_roll_in already does.
minimal_diff: Proposed only; unapplied and uncompiled.
```diff
--- a/crates/shortwin-math/src/pool.rs
+++ b/crates/shortwin-math/src/pool.rs
@@ -1906,6 +1906,9 @@
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

LEAD | program: shortwin | function: sync_lender | bug_class: repeated-accrual-truncation | group_key: shortwin | sync_lender | repeated-accrual-truncation
title: Repeated sync calls discard fractional lender credits
location: programs/shortwin/src/instructions/sync_lender.rs:119-130
related_locations: crates/shortwin-math/src/pool.rs:1459-1502,1660-1695
code_smells: touch_tail floors each current-epoch accrual and then advances both snapshots. An unsigned caller can invoke that path for another lender.
description: An outsider can discard a lender's fractional credits through repeated sync calls; this review does not establish a profitable transaction schedule.
invariant: For fixed shares, splitting a lender update should preserve its accumulated entitlement and realised allocation within a single final rounding unit.
proof: For a current lender with 1000 of 3000 shares, three separate one-lamport global credits produce accumulator increments of 333333333333333, 333333333333333, and 333333333333334. A touch after each credits floor(1000*increment/10^18)=0 every time and overwrites snap. A single touch after all three credits one lamport. Over 300 one-lamport credits, separate touches give zero while one final touch gives 100 lamports. The same arithmetic applies to acc_r when those credits are realised. sync_lender with owner.is_signer=false calls touch_pot, which reaches touch_tail for a current lender. The rebase authorization guard does not prevent this current-epoch path.
guards_and_limits: Repeating sync without an intervening accumulator change is idempotent. The forward-chain path uses cumulative floors and avoids this particular loss. This review does not establish live credit timing, transaction costs, or a direct attacker payout from the lost entitlement. Source comments prescribe floor rounding, but do not establish that outsiders should erase a lender's accumulated fractions repeatedly.
fix: Preserve each lender's fractional accrual remainder, or use cumulative entitlement differences across snapshots without a share change.

LEAD | program: shortwin | function: deposit | bug_class: donation-extends-deposit-hold | group_key: shortwin | deposit | donation-extends-deposit-hold
title: Small donations can extend a settlement hold
location: programs/shortwin/src/instructions/deposit.rs:130-135
related_locations: programs/shortwin/src/instructions/sol_settle.rs:336-342; programs/shortwin/src/instructions/sweep_trader.rs:351-383; crates/shortwin-math/src/pool.rs:948-999,2200-2230
code_smells: A settlement arms deposit_hold_until, but deposit tests the shared vest balance. Anyone can add another lump through sweep_trader.
description: An outsider can extend a settlement's deposit hold with small donations; this review leaves the intended donation policy unresolved.
invariant: A hold introduced for pre-deposit settlement income should end after that income releases, unless the protocol explicitly gives outsiders control over the hold.
proof: Consider S>0, deposit_hold_until>=0, both sweep bits already set, and a settlement vest whose balance has one lamport left. With lump_vest_s=21600, at one second before vest_end the old one-lamport balance releases zero. A one-lamport trader donation followed by sweep_trader adds a second lamport and computes the new remaining span as floor((1*1+1*21600)/2)=10800 seconds. deposit continues to fail PoolDrained while vest_bal>0, irrespective of the original recorded hold end. A donation at the original end can release the old balance and start a new one-lamport vest before a deposit runs. Repetition requires successful transaction ordering, but does not require a lender or borrower signature.
guards_and_limits: A pending first trader sweep after a dust flush uses immediate credit and does not extend the vest. The trace starts after that source bit is set, or from an ordinary held settlement. last-share withdrawal and a subsequent full vest flush can end the hold. Source comments deliberately require the shared vest to reach zero because later income can extend its end; they do not resolve whether arbitrary donations may perpetuate that restriction. This remains a policy-sensitive lead.
fix: Track the release of settlement income separately from new discretionary donations before the program releases the deposit hold.

LEAD | program: shortwin | function: deposit | bug_class: hardening-pool-vault-fields-unchecked | group_key: shortwin | deposit | hardening-pool-vault-fields-unchecked
title: Pool vault loads omit token field constraints
location: programs/shortwin/src/instructions/deposit.rs:94-99
related_locations: programs/shortwin/src/instructions/withdraw.rs:127-132; programs/shortwin/src/instructions/create_market.rs:281-350
code_smells: The pool_vault loader checks its PDA and token-account type, but omits token::mint, token::authority, and token::token_program constraints.
description: A future custody change could expose deposits to incorrect vault fields because the loader relies on initialization to preserve them.
proof: Deposit pins pool_vault to the market's recorded address and PDA. create_pool_vault creates it with the chosen token program and calls initialize_account3 with the listed mint and market authority. Those creation checks currently establish custody. The later Deposit and Withdraw pool_vault fields specify only mut, seeds and bump. The owner's separate token account has token::mint and token::token_program constraints. No current path that changes the canonical vault's mint or authority was established.
guards_and_limits: An outsider cannot substitute an arbitrary token account at this PDA. The typed loader accepts only token-program-owned token accounts. transfer_checked also checks its transfer-specific token properties. The current initialization and custody model blocks a demonstrated exploit; this is a defensive check gap only.
fix: Add the mint, authority, and token-program constraints directly to each pool_vault account field.
