# PoC 02_open_short

- Key: `shortwin|open-short|stale-ema-open-margin`
- Label: **CONFIRMED**
- Note: open_short accepts a 60-second-old P_ema, so the position holds 9.05 SOL but the borrowed tokens cost 27.35 SOL; lenders can lose 18.3 SOL.
- Test: `poc_02_open_short.rs`. It is a LiteSVM integration test of the `shortwin-litesvm-tests` package. Put it in `tests/litesvm/tests/` of a short.win 1.0.1 checkout, build the programs as that package's README says, and run the command below from the checkout's root.
- Command: `cargo test -p shortwin-litesvm-tests --test poc_02_open_short -- --nocapture`

## Result of the PoC run

```text
running 1 test
[setup] P_ema(listing)=1000000000000000  ema_ts=1790000000  now=1790000060  b_lo=0  open_positions=0
[pushed] live spot=9013233082709981 (~9x P_ema)  B_hat(q) at live price = 27353650775 lamports (~27.354 SOL)
[attack] P_ema=1000000000000000 (stale)  spot after sell-down=998531277913235 (14 bps off P_ema)  ema_ts=1790000000  now=1790000060  age=60s  COVERAGE_GAP_S=40  b_lo=0
[1.0.1: open LANDED] q=3000000000000 s=2979209197 m=6070496543 V=S+M=9049705740 n_open(B_spot)=2994163979 b_ema=3016568580 tier=2
  margin locks V=9.050 SOL, priced at the stale-low P_ema; the q borrowed tokens are worth B_hat=27.354 SOL at the live price
  => the position is under-collateralised; the lenders cannot recover ~18.304 SOL (18303945035 lamports) once the price is restored
panicked at tests/litesvm/tests/poc_02_open_short.rs:160:5:
assertion `left == right` failed: open_short accepted a short against a P_ema 60s old (> COVERAGE_GAP_S=40): it locked the D4 margin at the stale-low P_ema while the 3000000000000 borrowed tokens are worth ~27.4 SOL at the live price, so the lenders bear the shortfall (open landed: true)
  left: None
 right: Some(6101)
test open_short_accepts_a_stale_ema_and_undercollateralises_the_lenders ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s
```

## Faithfulness (the PoC agent's account)

Each step maps to the finding's attack path, driven only by real instructions; the only directly written state is the mock pool reserves (via real swaps) and the clock (via warp).

1. "a market holds lender tokens and has no open position; nothing sends crank_price" -> setup_with(spec) lists a mock market through the real create_market + lender deposit, then never cranks. create_market seeds the observation with Observation::new (empty bucket, ema_ts=now); the test asserts open_positions==0 and b_lo==0. This idleness is faithful: services/keeper/src/decide.rs::market_needs grants a position-less market no per-second cadence, so a correct keeper issues no crank.
2. "P_ema stays at its last integration while the pool price rises F times" -> a real whale buy (env.push_price, QuoteIn on the mock pool) moves the live spot to ~9x P_ema; no crank follows. The clock sits at creation+60 (spec.warmup=60, a warp), so ema_ts is 60 s old (> COVERAGE_GAP_S=40). The whale buy and the clock advance both stand for ordinary market activity and passage of time, the one allowance the guide permits.
3. "sell (sqrt(F)-1)*X0 tokens into the pool so the spot meets the old P_ema" -> a real attacker sale (env.push_price, BaseIn) brings the spot to within 14 bps of P_ema (asserted <= open_ema_dev_bps=1000). The attacker holds its own token inventory (the finding's "returned in the bundle").
4. "open_short ... spot_near_ema passes ... N_open and B_ema quoted at the pushed-down price" -> the real open_short instruction (open_short_ix, a fresh borrower paying the margin, mock ReadSet) is sent to a clone of the VM (env.svm.clone()) solely to read its outcome without perturbing the measured pool; this mirrors the harness's own Lab::try_send. The Open event confirms the margin was priced at the stale-low P_ema: V=S+M=9.05 SOL, n_open(B_spot)=2.99 SOL, b_ema=3.02 SOL.

Harm metric: V (Open event) vs B_hat(q)=quote_buy at the live price (the honest cost to buy back the q borrowed tokens once the pool is restored) = 27.35 SOL; shortfall = B_hat - V = 18.3 SOL is the lenders' loss (slightly conservative, it ignores the K_s the keeper takes). This is the finding's mechanism: the D4 margin covers only cr_open*B_hat at the stale price, ~F/cr_open too little.

Why the mock venue is faithful to the audited binary: open_short.rs is byte-identical to the audited e035f2e release, and the missing age check is in the venue-independent step-4 open guard. The Raydium/PumpSwap ema_warm blocks are skipped for the mock, so the mock isolates exactly the missing P_ema-age check. No short.win account (market, observation, ema_ts, lender vault) is written directly; all reach their state through real instructions. The empty bucket (b_lo==0) means the open's prologue roll integrates nothing, so ema_ts stays 60 s old at the step-4 guard - exactly the state the Fix's require!(now - market.ema_ts <= COVERAGE_GAP_S) refuses.

## Adversarial review

Verdict: kept. My re-run gives the same values as the report: the real open_short lands against a P_ema that is 60 s old (empty bucket, sold-down spot 14 bps from P_ema). The test then fails at the final assertion (line 160, left None, right Some(6101) EmaWarmingUp), because open_short.rs never reads ema_ts and the keeper's market_needs/observation_due send no crank to a market with no position. The path is faithful: the source is identical to e035f2e, and the only direct writes are the pool swaps, the clock warp and the attacker's minted tokens; two caveats are that it runs on the test-venue mock (the step-4 guard does not depend on the venue) and that the harm is computed (V = 9.05 SOL against B_hat(q) = 27.35 SOL at the live price) without running the buyback and S1, though V is all of the position's collateral.

```text
[setup] P_ema(listing)=1000000000000000  ema_ts=1790000000  now=1790000060  b_lo=0  open_positions=0
[pushed] live spot=9013233082709981 (~9x P_ema)  B_hat(q) at live price = 27353650775 lamports (~27.354 SOL)
[attack] P_ema=1000000000000000 (stale)  spot after sell-down=998531277913235 (14 bps off P_ema)  ema_ts=1790000000  now=1790000060  age=60s  COVERAGE_GAP_S=40  b_lo=0
[1.0.1: open LANDED] q=3000000000000 s=2979209197 m=6070496543 V=S+M=9049705740 n_open(B_spot)=2994163979 b_ema=3016568580 tier=2
  => the position is under-collateralised; the lenders cannot recover ~18.304 SOL (18303945035 lamports) once the price is restored
panicked at tests/litesvm/tests/poc_02_open_short.rs:160:5: assertion `left == right` failed ... (open landed: true)
  left: None
 right: Some(6101)
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s
```
