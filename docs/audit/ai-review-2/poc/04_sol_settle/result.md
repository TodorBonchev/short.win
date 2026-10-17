# PoC 04_sol_settle

- Key: `shortwin|sol-settle|vested-principal-diluted-by-late-deposit`
- Label: **CONFIRMED**
- Note: After an S3 on a halted pool, a new depositor gets its tokens back and takes 5.87 of the old lender's 11.71 vested SOL.
- Test: `poc_04_sol_settle.rs`. It is a LiteSVM integration test of the `shortwin-litesvm-tests` package. Put it in `tests/litesvm/tests/` of a short.win 1.0.1 checkout, build the programs as that package's README says, and run the command below from the checkout's root.
- Command: `cargo test -p shortwin-litesvm-tests --test poc_04_sol_settle -- --nocapture`

## Result of the PoC run

```text
Running tests/poc_04_sol_settle.rs (target/debug/deps/poc_04_sol_settle-0a09ecb9e9bd77a0)
running 1 test
X deposits 1000000000000 tokens after the S3 at T = 996850878012, S = 1000000000000000: 1003159070285695 shares, 0.5008 of the pool's shares
S3 on the halted pool: q 3149121988 tokens extinguished, V 15039559560 lamports, lenders' take 11711842814, R_due 285389, lump 11711557425, vest 11711752240 lamports (lump_vest_s 21600 s)
control (no late deposit): H is paid 11711842813 lamports after the vest
attack: X (deposit 1000000000000 tokens after the S3, share 0.5008) is paid 5865111095 lamports and gets 999999999999 of 1000000000000 tokens back; H is paid 5846731717 lamports
H loses 5865111096 lamports; X gains 5865111095 lamports (50.08 % of the vest)

thread 'a_deposit_after_a_halted_venue_s3_takes_none_of_the_old_lenders_vest' (248686) panicked at tests/litesvm/tests/poc_04_sol_settle.rs:238:5:
harm: X, who deposited 1000000000000 tokens after the halted-venue S3 and got 999999999999 of them back, takes 5865111095 lamports of the 11711752240-lamport vest that pays for H's 3149121988 extinguished tokens; H is paid 5846731717 instead of 11711842813 lamports (loses 5865111096)
test a_deposit_after_a_halted_venue_s3_takes_none_of_the_old_lenders_vest ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.79s

(binary under test: target/deploy/shortwin.so, sha256 72c82879accac8323af6ef0ce777baea6b40c1ce312cefaf65aaa152a3f21429, SHORTWIN_SO unset, so Env::new loads the audited release build)
```

## Faithfulness (the PoC agent's account)

Program and venue: the audited release build (Env::new loads target/deploy/shortwin.so, sha256 72c82879...429) runs against the dumped Raydium AMM v4 program and Fartcoin's pool. The scenario reuses raydium_v4_e2e::admin_disable_reaches_s3_after_the_timer through raydium_v4_support.rs. The invariant hooks run after every instruction.

Step-by-step mapping to the finding's path:
1. Lender H: Fart::new lists the market and H deposits 1e12 tokens, both through real instructions that the harness checks.
2. Position: a borrower opens about 5 SOL of Fartcoin with a real open_short. A whale's real AMM v4 buys pump the price x2.3, so the position becomes liquidatable. These buys stand for other market participants.
3. Halted venue (the finding's precondition): the Raydium pool admin's real SetParams instruction (status 4, LiquidityOnly) halts the pool. The harness forges the admin's signature. This stands for a third party, the venue admin, halting the pool. The attacker does not do it.
4. S3: the clock moves in 30 s steps, with a keeper crank_price at each step. The honest keeper's permissionless sol_settle executes the S3. settle_checked checks it against shortwin-math: condition 3, not executable, so p_imm = 0 (sol_settle.rs:285). The test asserts lump = take - R_due, vest_flushed = 0, and no position open after it (borrowed_q = 0, so the AIR-07 deposit gate does not apply).
5. Control, from the same state just before the S3: the S3 alone, a warp of lump_vest_s + 1 s, a crank, then H claims twice. H gets 11,711,842,813 lamports, which is the whole vest.
6. Attack: the same S3 (the test asserts identical amounts and vest). In the same second, X sends a plain deposit of 1e12 tokens in its own transaction. X needs no bundle and no keeper role, so this attacker is weaker than the finding's [sol_settle, deposit] bundle.
7. X holds for lump_vest_s + 1 s (passage of time), and one keeper crank_price accrues the release. X then sends [claim_sol, withdraw] for the most shares its allowance admits, and claims twice. H claims twice.
8. Measurement: SOL comes from the ClaimSol and Withdraw events' sol_paid; tokens come from the Withdraw events. No position is open during the hold, so the vest is the only SOL source. H's loss (5,865,111,096) equals X's gain (5,865,111,095), and X gets 999,999,999,999 of 1e12 tokens back.

Direct state writes:
- X's 1e12 Fartcoin balance: fork::fund_token creates X's token account and plants the balance in it. This stands for X buying Fartcoin earlier; the mainnet mint has no mint authority. The harness funds the honest lender the same way.
- The clock warps stand for the passage of time.
- No short.win account is written directly, and no venue bytes are written. The halt is the admin's real instruction.

Regression shape: the final assertion is x_sol <= 2 && h_attack + 2 >= h_control (the safe behaviour). If deposit is refused (the Fix's deposit_hold_until), the test counts X's take as 0, so it passes after the fix. Every earlier step is checked with ok(), expect or assert.

## Adversarial review

Verdict: kept. My re-run of the audited release build (sha256 72c82879...429, SHORTWIN_SO unset, program and crate sources identical to e035f2e) fails at the final assertion (line 238) with the agent's exact numbers: after a checked S3 on a pool halted by the Raydium admin's real SetParams, X's plain deposit of 1e12 tokens takes 5,865,111,095 of the 11,711,752,240-lamport vest and gets 999,999,999,999 tokens back, while H falls from 11,711,842,813 to 5,846,731,717 lamports. Every step is a real instruction; the only direct writes are X's token balance, SOL airdrops, the forged admin signature for the third-party halt and clock warps, and the path matches the code (sol_settle.rs:285 p_imm = 0, pool.rs:2356 no flush, the deposit_in gate needs borrowed_q > 0, release_in credits every current share). The loss is mostly principal, not premium (FV's premium is only the 5-10 % tier penalty plus fees), which ECON 9.4 R4B-6 assigns to the lenders who held, and KI-24 accepts only a whale that deposits before the S3.

```text
72c82879accac8323af6ef0ce777baea6b40c1ce312cefaf65aaa152a3f21429  target/deploy/shortwin.so   (SHORTWIN_SO=[unset])
     Running tests/poc_04_sol_settle.rs (target/debug/deps/poc_04_sol_settle-0a09ecb9e9bd77a0)
X deposits 1000000000000 tokens after the S3 at T = 996850878012, S = 1000000000000000: 1003159070285695 shares, 0.5008 of the pool's shares
S3 on the halted pool: q 3149121988 tokens extinguished, V 15039559560 lamports, lenders' take 11711842814, R_due 285389, lump 11711557425, vest 11711752240 lamports (lump_vest_s 21600 s)
control (no late deposit): H is paid 11711842813 lamports after the vest
attack: X (deposit 1000000000000 tokens after the S3, share 0.5008) is paid 5865111095 lamports and gets 999999999999 of 1000000000000 tokens back; H is paid 5846731717 lamports
H loses 5865111096 lamports; X gains 5865111095 lamports (50.08 % of the vest)
thread 'a_deposit_after_a_halted_venue_s3_takes_none_of_the_old_lenders_vest' (249303) panicked at tests/litesvm/tests/poc_04_sol_settle.rs:238:5:
harm: X, who deposited 1000000000000 tokens after the halted-venue S3 and got 999999999999 of them back, takes 5865111095 lamports of the 11711752240-lamport vest that pays for H's 3149121988 extinguished tokens; H is paid 5846731717 instead of 11711842813 lamports (loses 5865111096)
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.75s
```
