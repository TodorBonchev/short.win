FINDING | program: shortwin-math | function: withdraw_in | bug_class: carry-crosses-empty-pool | group_key: shortwin-math | withdraw_in | carry-crosses-empty-pool
location: crates/shortwin-math/src/pool.rs:1908 (shortwin-math::withdraw_in)
title: The last withdrawal leaves SOL carry for the next lender
confidence: 97
root_cause: withdraw_in leaves carry and carry_r intact when total_shares reaches zero, so a later cohort receives the previous cohort's undistributed SOL.
invariant: A new lender must not acquire the old cohort's accrued or realised SOL after every old share has been withdrawn.
violation_path: Old lender deposits -> unrelated income enters through sweep_trader -> old lender withdraws all shares and receives zero SOL -> sweep_trader clears the required source bit -> attacker deposits one raw token -> attacker donates one lamport and calls sweep_trader -> attacker withdraws all shares and receives the old income plus that lamport.
path: Use a live, list-only market with an accepted ordinary SPL mint, no borrowed tokens, deposit_hold_until = -1, and initially empty lender accounting. The old owner signs its deposit and withdrawal. The attacker signs only its own deposit, one-lamport System transfer, and withdrawal; sweep_trader requires no authority signature. Pass the genuine market, vaults, mint, token program, trader and trader_wsol accounts. Neither owner needs share-epoch records. The attacker's withdrawal passes min_tokens = 1 and min_sol = 1000 in the first example.
proof: All arithmetic below uses the frozen production source; no build, test, or executable PoC was run.
  1. Let WAD = 10^18, VA = 1, VS = 1000. The old lender deposits A = 10^18 raw tokens. shares_for_tokens returns A*1000 = 10^21 shares. Thus T=A, B=0, S=10^21, acc=acc_r=carry=carry_r=0.
  2. An unrelated income payer sends X=999 lamports to the market's trader. sweep_trader normalises those actual lamports into MarketVault and calls pool::sweep (instructions/sweep_trader.rs:279 onward; pool.rs:2200 onward). lender_sol=999 and vest_bal=999. This uses actual deposited SOL, not a fabricated counter. There is no minimum sweep amount. The transfer and sweep can share one transaction so the trader ends with zero lamports.
  3. Before the vest ends, the old lender withdraws all S shares. tokens_for_shares returns floor((1000*A)*(A+1)/(1000*A+1000))=A. B=0, so the production LastSharesBorrowed guard passes. withdraw_in flushes the 999-lamport vest at pool.rs:1897-1904. credit_pos computes floor(999*10^18/10^21)=0 and carry=999*10^18; credit_r_in computes the same zero delta and carry_r=999*10^18. The old lender's pending and real both remain zero. The function sets S=0 and clears only wipe_swept, at lines 1906-1912. Both carries remain.
  4. The production withdraw handler immediately calls pool::claim (instructions/withdraw.rs:212-215). It pays zero because pending=real=claimed=0. pool::closable is true: no shares, no pending, no lock, and current epoch. The handler closes the lender account. A claim before this withdrawal also pays zero because both index deltas are zero. A claim after the close cannot recover the 999 lamports. Setting min_sol=999 only makes the old withdrawal fail; it does not expose a path that claims the carry.
  5. The attacker calls a zero sweep. For the list-only venue only the trader source bit is required. No SOL moves, no live wipe record exists, and the carry remains. The attacker deposits one raw token, which mints 1000 shares against T=B=S=0. Its initial snapshots are acc=acc_r=0. The existing lender_sol remains 999.
  6. The attacker transfers one actual lamport to trader and calls sweep_trader. The trader bit is already set, so the new lamport vests normally: lender_sol=1000, vest_bal=1. It then withdraws all its 1000 shares before the new vest ends. Its full exit flushes the one lamport. Both index numerators equal 1*WAD + 999*WAD = 1000*WAD. Division by S=1000 gives delta=WAD. The lender touch credits pending=real=1000, and both carries become zero. pool::claim pays all 1000 lamports. Its raw token also returns in full. Its net SOL receipt before transaction fees is the old owner's 999 lamports.
  7. The pot is zero throughout: unmatched=0, and no pot draw is required. This is cross-cohort index allocation, not recovery of a stranded pot. MarketVault's actual lamports always equal its rent plus the tracked SOL, so its rent-floor and backing checks do not reject the path.
production_bounds: deposit accepts amount:u64 and checks amount>0, minted>0, min_shares, arithmetic, mint safety, and pool conditions; it imposes no smaller token amount cap. The initial A=10^18 is below u64::MAX. mint_filter::check_mint_safety (programs/shortwin/src/mint_filter.rs:64-100) does not cap mint supply or decimals. An initialized non-native SPL mint with no freeze authority and supply at least A suffices. A list-only market permits deposits, withdrawals and trader sweeps, so open caps and min_notional do not participate. For a larger fully traced instance, A=18,000,000,000,000,000,000 also fits u64, S=18,000,000,000,000,000,000,000, and X=17,999 satisfies X*WAD<S. The same path pays the next lender 18,000 lamports after its one-lamport donation, a 17,999-lamport cross-cohort transfer before fees. The initial-deposit case has X < 1000*A/WAD, so at the u64 token limit this specific construction transfers at most 18,446 whole lamports per empty-pool transition. Higher share/token ratios can increase the representation quantum, but this report does not assert their maximum reachability or profit.
intent_check: pool.rs documents global carry as an undistributed remainder in [0,S), and explicitly drops carries into dust when an epoch rolls (maybe_roll_in). Its last-withdraw documentation says the last lender receives the unreleased vest. The zero-share withdrawal branch neither pays nor drops the carries. No read source states that the next depositor owns the previous cohort's carry. This is larger than one-lamport rounding dust, but its bounded magnitude remains relevant to final triage.
description: A new lender takes the previous lender's SOL because the last withdrawal leaves both index carries in the empty pool.
fix: Pay whole-lamport carry to the last lender and clear both carries before the next deposit.
minimal_diff: This proposed diff pays whole units to the last holder and drops only sub-lamport residues, with the existing checked helpers; it is untested.
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

LEAD | program: shortwin | function: deposit | bug_class: donation-extends-deposit-hold | group_key: shortwin | deposit | donation-extends-deposit-hold
location: programs/shortwin/src/instructions/deposit.rs:130-135; crates/shortwin-math/src/pool.rs:965-986
program_function: shortwin::deposit
title: Small donations keep a settlement hold active
code_smells: The deposit hold requires an empty shared vest; any caller adds a new vest through a trader sweep.
description: An attacker extends a deposit hold with one-lamport sweeps; a safe fix must preserve the protection for historical rewards and split claims.
invariant: A hold that protects a completed settlement should eventually permit deposits despite unrelated later donations.
partial_path: A settlement leaves S>0 and a vest without a dust flush; sol_settle.rs:327-328 arms deposit_hold_until. An attacker sends a one-lamport System transfer to trader, followed by sweep_trader before each vest expiry. The public sweep adds to the same vest that deposit requires to reach zero.
concrete_trace: Let a held vest contain 1,000,000 lamports and span 21,600 seconds, with both source bits already set and no other income or withdrawals. At t=end-1, the prologue releases floor(1,000,000*21,599/21,600)=999,953, leaving 47. A one-lamport sweep computes floor((47*1+1*21,600)/(47+1))=450, so vest_bal=48 and the new end is t+450. At the new end minus one second, release leaves ceil(48/450)=1. Another one-lamport sweep gives vest_bal=2 and a new end t+floor((1+21,600)/2)=t+10,800. Each later sweep at end-1 leaves one old lamport and adds one new lamport, so the 10,800-second cycle repeats. No sweep races a completed vest; each can occur before expiry while deposits already fail. Costs are one lamport and transaction fees per cycle. The instruction contains no signer, minimum amount, or cooldown that prevents it.
related_path: Once the original vest has fully released, an attacker can also sweep a fresh one-lamport donation before the first successful deposit. Only deposit clears deposit_hold_until to -1. The fresh vest then blocks that deposit for another lump_vest_s; this variant requires transaction ordering at reopening.
recoveries_and_limits: Withdrawals, claims and position exits remain available. The last lender can flush the vest through a full withdrawal when no tokens remain borrowed. A full wipe also flushes it. A successful deposit during an unprotected gap clears the hold. The trace does not prove a theft or a permanent block after the attacker stops. The specific 1,000,000-lamport starting vest is a valid pool state; a full production settlement transaction that produces that exact state was not constructed.
intent_and_open_question: Market.deposit_hold_until and deposit.rs expressly state that later credits can extend the shared vest, so this lead preserves that stated design rather than treating extension alone as a defect. The low-cost recurrent donation path needs an explicit liveness decision. Simply replacing vest_bal==0 with now>=deposit_hold_until would reintroduce historical reward dilution; clearing a completed marker does not fix the pre-expiry cycle. No safe minimal diff is established here. A separate protected tranche or immediate allocation of qualifying post-hold sweeps needs accounting review across sweep_trader and every trade normalisation before promotion.
